use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// Recorded invocation of `git` or `gh` for transparency and debugging.
#[derive(Clone, Debug)]
pub struct CommandEntry {
    pub program: String,
    pub args: Vec<String>,
    pub duration_ms: u64,
    pub ok: bool,
    pub stdout_preview: String,
    pub stderr: String,
    pub timestamp: DateTime<Local>,
}

impl CommandEntry {
    pub fn display_cmd(&self) -> String {
        let mut parts = Vec::with_capacity(self.args.len() + 1);
        parts.push(self.program.clone());
        for arg in &self.args {
            if arg.contains(' ') || arg.contains('\t') || arg.contains('\n') || arg.is_empty() {
                let escaped = arg.replace('"', "\\\"");
                parts.push(format!("\"{escaped}\""));
            } else {
                parts.push(arg.clone());
            }
        }
        parts.join(" ")
    }
}

/// Runs `program` with `args` inside `cwd` without interactive prompts and records a `CommandEntry`.
pub fn run_cmd(program: &str, args: &[&str], cwd: &Path) -> (Result<String>, CommandEntry) {
    let started = Instant::now();
    let timestamp = Local::now();

    let output_res = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .output()
        .with_context(|| format!("Failed to spawn `{program}`"));

    let duration_ms = started.elapsed().as_millis() as u64;
    let args_owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();

    match output_res {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let ok = output.status.success();
            let stdout_preview = truncate_preview(&stdout, 320);

            let entry = CommandEntry {
                program: program.to_string(),
                args: args_owned,
                duration_ms,
                ok,
                stdout_preview,
                stderr: stderr.clone(),
                timestamp,
            };

            if ok {
                (Ok(stdout), entry)
            } else {
                let msg = if stderr.is_empty() {
                    format!("`{}` exited with {}", entry.display_cmd(), output.status)
                } else {
                    stderr
                };
                (Err(anyhow!(msg)), entry)
            }
        }
        Err(err) => {
            let err_str = format!("{err:#}");
            let entry = CommandEntry {
                program: program.to_string(),
                args: args_owned,
                duration_ms,
                ok: false,
                stdout_preview: String::new(),
                stderr: err_str,
                timestamp,
            };
            (Err(err), entry)
        }
    }
}

fn truncate_preview(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.replace('\n', " ↵ ")
    } else {
        let head: String = trimmed.chars().take(max_chars).collect();
        format!("{}…", head.replace('\n', " ↵ "))
    }
}
