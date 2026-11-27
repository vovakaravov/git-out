use std::fmt::Write;
use std::ops::RangeInclusive;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchLineKind {
    Context,
    Added,
    Removed,
    NoNewline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchLine {
    pub kind: PatchLineKind,
    /// Line text without the leading `+` / `-` / ` ` marker and without trailing `\n`.
    pub text: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedHunk {
    pub index: usize,
    pub header: String,
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<PatchLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedFilePatch {
    pub old_path: String,
    pub new_path: String,
    pub header_lines: Vec<String>,
    pub hunks: Vec<ParsedHunk>,
}

#[cfg(test)]
impl ParsedFilePatch {
    pub fn display_path(&self) -> &str {
        if self.new_path != "/dev/null" && !self.new_path.is_empty() {
            &self.new_path
        } else {
            &self.old_path
        }
    }
}

/// Parses a unified git diff (one or more files) into structured hunks and 1-based line numbers.
pub fn parse_unified_patch(patch: &str) -> Vec<ParsedFilePatch> {
    let mut files = Vec::new();
    let mut cur_file: Option<ParsedFilePatch> = None;
    let mut cur_hunk: Option<ParsedHunk> = None;
    let mut old_ln = 0usize;
    let mut new_ln = 0usize;

    let flush_hunk = |cur_file: &mut Option<ParsedFilePatch>, cur_hunk: &mut Option<ParsedHunk>| {
        if let (Some(file), Some(hunk)) = (cur_file.as_mut(), cur_hunk.take()) {
            file.hunks.push(hunk);
        }
    };

    for raw_line in patch.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        if line.starts_with("diff --git ") {
            flush_hunk(&mut cur_file, &mut cur_hunk);
            if let Some(prev) = cur_file.take() {
                files.push(prev);
            }
            let (old_p, new_p) = parse_diff_git_paths(line);
            cur_file = Some(ParsedFilePatch {
                old_path: old_p,
                new_path: new_p,
                header_lines: vec![line.to_string()],
                hunks: Vec::new(),
            });
            continue;
        }

        let Some(file) = cur_file.as_mut() else {
            continue;
        };

        if let Some((os, oc, ns, nc)) = parse_hunk_header(line) {
            flush_hunk(&mut cur_file, &mut cur_hunk);
            let file = cur_file.as_mut().unwrap();
            let idx = file.hunks.len();
            old_ln = os;
            new_ln = ns;
            cur_hunk = Some(ParsedHunk {
                index: idx,
                header: line.to_string(),
                old_start: os,
                old_count: oc,
                new_start: ns,
                new_count: nc,
                lines: Vec::new(),
            });
            continue;
        }

        if let Some(hunk) = cur_hunk.as_mut() {
            if let Some(rest) = line.strip_prefix('+') {
                hunk.lines.push(PatchLine {
                    kind: PatchLineKind::Added,
                    text: rest.to_string(),
                    old_line: None,
                    new_line: Some(new_ln),
                });
                new_ln += 1;
            } else if let Some(rest) = line.strip_prefix('-') {
                hunk.lines.push(PatchLine {
                    kind: PatchLineKind::Removed,
                    text: rest.to_string(),
                    old_line: Some(old_ln),
                    new_line: None,
                });
                old_ln += 1;
            } else if line.starts_with("\\ No newline at end of file") {
                hunk.lines.push(PatchLine {
                    kind: PatchLineKind::NoNewline,
                    text: line.to_string(),
                    old_line: None,
                    new_line: None,
                });
            } else if let Some(rest) = line.strip_prefix(' ') {
                hunk.lines.push(PatchLine {
                    kind: PatchLineKind::Context,
                    text: rest.to_string(),
                    old_line: Some(old_ln),
                    new_line: Some(new_ln),
                });
                old_ln += 1;
                new_ln += 1;
            } else {
                hunk.lines.push(PatchLine {
                    kind: PatchLineKind::Context,
                    text: line.to_string(),
                    old_line: Some(old_ln),
                    new_line: Some(new_ln),
                });
                old_ln += 1;
                new_ln += 1;
            }
        } else {
            if let Some(p) = line.strip_prefix("--- ") {
                file.old_path = clean_header_path(p);
            } else if let Some(p) = line.strip_prefix("+++ ") {
                file.new_path = clean_header_path(p);
            }
            file.header_lines.push(line.to_string());
        }
    }

    flush_hunk(&mut cur_file, &mut cur_hunk);
    if let Some(last) = cur_file.take() {
        files.push(last);
    }

    files
}

/// Builds a surgical patch for lines in a UI `DiffLineRange` (handling both single-side and
/// cross-side Unified selections regardless of drag direction).
pub fn build_patch_for_diff_range(
    file: &ParsedFilePatch,
    start_is_old: bool,
    start_line: usize,
    end_is_old: bool,
    end_line: usize,
    reverse: bool,
) -> Option<String> {
    if start_is_old == end_is_old {
        let lo = start_line.min(end_line);
        let hi = start_line.max(end_line);
        return if start_is_old {
            build_selected_lines_patch(file, Some(&(lo..=hi)), None, reverse)
        } else {
            build_selected_lines_patch(file, None, Some(&(lo..=hi)), reverse)
        };
    }

    // Cross-side selection in Unified view: track patch-order positions to find all covered old/new lines.
    let mut flat: Vec<(usize, &PatchLine)> = Vec::new();
    let mut start_pos = None;
    let mut end_pos = None;

    for hunk in &file.hunks {
        for line in &hunk.lines {
            let idx = flat.len();
            flat.push((idx, line));
            let matches = |is_old: bool, ln: usize| match line.kind {
                PatchLineKind::Removed => is_old && line.old_line == Some(ln),
                PatchLineKind::Added => !is_old && line.new_line == Some(ln),
                PatchLineKind::Context => {
                    if is_old {
                        line.old_line == Some(ln)
                    } else {
                        line.new_line == Some(ln)
                    }
                }
                PatchLineKind::NoNewline => false,
            };
            if matches(start_is_old, start_line) {
                start_pos = Some(idx);
            }
            if matches(end_is_old, end_line) {
                end_pos = Some(idx);
            }
        }
    }

    let (Some(a), Some(b)) = (start_pos, end_pos) else {
        return None;
    };
    let (lo, hi) = (a.min(b), a.max(b));
    let mut old_min = usize::MAX;
    let mut old_max = 0usize;
    let mut new_min = usize::MAX;
    let mut new_max = 0usize;
    for &(_, line) in &flat[lo..=hi] {
        if let Some(ol) = line.old_line {
            old_min = old_min.min(ol);
            old_max = old_max.max(ol);
        }
        if let Some(nl) = line.new_line {
            new_min = new_min.min(nl);
            new_max = new_max.max(nl);
        }
    }

    let old_r = (old_min <= old_max).then_some(old_min..=old_max);
    let new_r = (new_min <= new_max).then_some(new_min..=new_max);
    build_selected_lines_patch(file, old_r.as_ref(), new_r.as_ref(), reverse)
}

/// Builds a standalone unified patch containing only `hunk_idx` from `file`.
pub fn build_hunk_patch(file: &ParsedFilePatch, hunk_idx: usize) -> Option<String> {
    let hunk = file.hunks.get(hunk_idx)?;
    let mut out = String::new();
    for h in &file.header_lines {
        let _ = writeln!(out, "{h}");
    }
    write_hunk(&mut out, hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count, &hunk.lines);
    Some(out)
}

/// Builds a surgical patch for only the changed lines falling inside `selected_old` / `selected_new`.
///
/// When `reverse == false` (staging / discarding forward patch):
/// - unselected `+` lines are dropped
/// - unselected `-` lines are converted to `' '` context lines
///
/// When `reverse == true` (unstaging via `git apply --cached --reverse`):
/// - unselected `-` lines are dropped (keeping index intact)
/// - unselected `+` lines are converted to `' '` context lines (so reverse only removes selected `+`)
pub fn build_selected_lines_patch(
    file: &ParsedFilePatch,
    selected_old: Option<&RangeInclusive<usize>>,
    selected_new: Option<&RangeInclusive<usize>>,
    reverse: bool,
) -> Option<String> {
    let mut body = String::new();
    let mut any_hunk = false;

    for hunk in &file.hunks {
        let mut filtered: Vec<(char, &str)> = Vec::new();
        let mut old_count = 0usize;
        let mut new_count = 0usize;
        let mut has_change = false;
        let mut prev_emitted_changed_or_context = true;

        for line in &hunk.lines {
            match line.kind {
                PatchLineKind::Context => {
                    filtered.push((' ', &line.text));
                    old_count += 1;
                    new_count += 1;
                    prev_emitted_changed_or_context = true;
                }
                PatchLineKind::Added => {
                    let is_sel = line
                        .new_line
                        .is_some_and(|ln| selected_new.is_some_and(|r| r.contains(&ln)));
                    if is_sel {
                        filtered.push(('+', &line.text));
                        new_count += 1;
                        has_change = true;
                        prev_emitted_changed_or_context = true;
                    } else if reverse {
                        // In index -> stays as context so `--reverse` doesn't undo it.
                        filtered.push((' ', &line.text));
                        old_count += 1;
                        new_count += 1;
                        prev_emitted_changed_or_context = true;
                    } else {
                        prev_emitted_changed_or_context = false;
                    }
                }
                PatchLineKind::Removed => {
                    let is_sel = line
                        .old_line
                        .is_some_and(|ln| selected_old.is_some_and(|r| r.contains(&ln)));
                    if is_sel {
                        filtered.push(('-', &line.text));
                        old_count += 1;
                        has_change = true;
                        prev_emitted_changed_or_context = true;
                    } else if !reverse {
                        // Still present in HEAD/base -> keep as context for staging/discarding.
                        filtered.push((' ', &line.text));
                        old_count += 1;
                        new_count += 1;
                        prev_emitted_changed_or_context = true;
                    } else {
                        prev_emitted_changed_or_context = false;
                    }
                }
                PatchLineKind::NoNewline => {
                    if prev_emitted_changed_or_context {
                        filtered.push(('\\', &line.text));
                    }
                }
            }
        }

        if has_change {
            any_hunk = true;
            let _ = writeln!(
                body,
                "@@ -{},{} +{},{} @@",
                hunk.old_start, old_count, hunk.new_start, new_count
            );
            for (prefix, text) in filtered {
                if prefix == '\\' {
                    let _ = writeln!(body, "{text}");
                } else {
                    let _ = writeln!(body, "{prefix}{text}");
                }
            }
        }
    }

    if !any_hunk {
        return None;
    }

    let mut out = String::new();
    for h in &file.header_lines {
        let _ = writeln!(out, "{h}");
    }
    out.push_str(&body);
    Some(out)
}

fn write_hunk(
    out: &mut String,
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: &[PatchLine],
) {
    let _ = writeln!(out, "@@ -{old_start},{old_count} +{new_start},{new_count} @@");
    for line in lines {
        match line.kind {
            PatchLineKind::Context => {
                let _ = writeln!(out, " {}", line.text);
            }
            PatchLineKind::Added => {
                let _ = writeln!(out, "+{}", line.text);
            }
            PatchLineKind::Removed => {
                let _ = writeln!(out, "-{}", line.text);
            }
            PatchLineKind::NoNewline => {
                let _ = writeln!(out, "{}", line.text);
            }
        }
    }
}

fn parse_diff_git_paths(line: &str) -> (String, String) {
    let rest = line.strip_prefix("diff --git ").unwrap_or("").trim();
    if let Some((left, right)) = rest.split_once(" b/") {
        let old = clean_header_path(left);
        let new = clean_header_path(right);
        return (old, new);
    }
    let mut parts = rest.split_whitespace();
    let old = parts.next().map(clean_header_path).unwrap_or_default();
    let new = parts.next().map(clean_header_path).unwrap_or_default();
    (old, new)
}

fn clean_header_path(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"');
    if let Some(p) = trimmed.strip_prefix("a/") {
        p.to_string()
    } else if let Some(p) = trimmed.strip_prefix("b/") {
        p.to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    let rest = line.strip_prefix("@@ -")?;
    let end_marker = rest.find(" @@")?;
    let range_str = &rest[..end_marker];
    let (old_part, new_part) = range_str.split_once(" +")?;
    let (os, oc) = parse_span(old_part)?;
    let (ns, nc) = parse_span(new_part)?;
    Some((os, oc, ns, nc))
}

fn parse_span(s: &str) -> Option<(usize, usize)> {
    if let Some((start, count)) = s.split_once(',') {
        Some((start.parse().ok()?, count.parse().ok()?))
    } else {
        Some((s.parse().ok()?, 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PATCH: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,5 @@
 line1
-line2
+line2_mod
+line2_extra
 line3
 line4
@@ -10,3 +11,4 @@ fn tail() {
 a
 b
+c
 d
";

    #[test]
    fn parses_multiple_hunks_and_line_numbers() {
        let files = parse_unified_patch(SAMPLE_PATCH);
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.display_path(), "src/lib.rs");
        assert_eq!(f.hunks.len(), 2);

        let h0 = &f.hunks[0];
        assert_eq!(h0.old_start, 1);
        assert_eq!(h0.old_count, 4);
        assert_eq!(h0.new_start, 1);
        assert_eq!(h0.new_count, 5);
        assert_eq!(h0.lines[1].old_line, Some(2));
        assert_eq!(h0.lines[2].new_line, Some(2));
        assert_eq!(h0.lines[3].new_line, Some(3));
    }

    #[test]
    fn builds_single_hunk_patch() {
        let files = parse_unified_patch(SAMPLE_PATCH);
        let p = build_hunk_patch(&files[0], 1).unwrap();
        assert!(p.contains("diff --git a/src/lib.rs b/src/lib.rs"));
        assert!(p.contains("@@ -10,3 +11,4 @@"));
        assert!(!p.contains("line2_mod"));
    }

    #[test]
    fn builds_selected_lines_patch_forward_and_reverse() {
        let files = parse_unified_patch(SAMPLE_PATCH);
        // Forward: select only new line 3 (+line2_extra), skip old line 2 (-line2) and new line 2 (+line2_mod)
        let fwd = build_selected_lines_patch(&files[0], None, Some(&(3..=3)), false).unwrap();
        assert!(fwd.contains("@@ -1,4 +1,5 @@"));
        assert!(fwd.contains(" line2\n+line2_extra\n"));
        assert!(!fwd.contains("-line2\n"));
        assert!(!fwd.contains("+line2_mod\n"));
        assert!(!fwd.contains("@@ -10,3"));

        // Reverse (unstage only +line2_extra): unselected +line2_mod stays context, -line2 omitted
        let rev = build_selected_lines_patch(&files[0], None, Some(&(3..=3)), true).unwrap();
        assert!(rev.contains("@@ -1,4 +1,5 @@"));
        assert!(rev.contains(" line2_mod\n+line2_extra\n"));
        assert!(!rev.contains("-line2\n"));
    }

    #[test]
    fn builds_patch_for_cross_side_diff_range() {
        let files = parse_unified_patch(SAMPLE_PATCH);
        let p = build_patch_for_diff_range(&files[0], true, 2, false, 2, false).unwrap();
        assert!(p.contains("-line2\n+line2_mod\n"));
        assert!(!p.contains("+line2_extra\n"));
    }
}
