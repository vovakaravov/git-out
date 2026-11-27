use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
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
    run_cmd_with_env(program, args, &[], cwd)
}

/// Runs `program` with extra environment variables inside `cwd` without interactive prompts.
pub fn run_cmd_with_env(
    program: &str,
    args: &[&str],
    env_vars: &[(&str, &str)],
    cwd: &Path,
) -> (Result<String>, CommandEntry) {
    let started = Instant::now();
    let timestamp = Local::now();

    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1");
    for &(k, v) in env_vars {
        cmd.env(k, v);
    }

    let output_res = cmd
        .output()
        .with_context(|| format!("Failed to spawn `{program}`"));

    finish_cmd_output(program, args, started, timestamp, output_res)
}

/// Runs `program` piping `stdin_data` to its standard input inside `cwd`.
pub fn run_cmd_with_stdin(
    program: &str,
    args: &[&str],
    stdin_data: &str,
    cwd: &Path,
) -> (Result<String>, CommandEntry) {
    let started = Instant::now();
    let timestamp = Local::now();

    let output_res = (|| -> Result<std::process::Output> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn `{program}`"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_data.as_bytes())
                .with_context(|| format!("Failed writing stdin to `{program}`"))?;
        }

        child
            .wait_with_output()
            .with_context(|| format!("Failed waiting on `{program}`"))
    })();

    finish_cmd_output(program, args, started, timestamp, output_res)
}

fn finish_cmd_output(
    program: &str,
    args: &[&str],
    started: Instant,
    timestamp: DateTime<Local>,
    output_res: Result<std::process::Output>,
) -> (Result<String>, CommandEntry) {
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
