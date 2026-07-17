use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use crate::error::NinetyNineError;

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ProcessOutcome {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub timed_out: bool,
}

/// Runs a command with a wall-clock timeout, capturing stdout and stderr.
///
/// On timeout the process is killed and `timed_out` is set. The exit status
/// in that case is non-success (or a zero placeholder if unavailable).
///
/// # Errors
///
/// Returns `RunnerExecution` if the process cannot be spawned or waited on.
pub fn run_timed(spec: &CommandSpec, timeout: Duration) -> Result<ProcessOutcome, NinetyNineError> {
    let start = Instant::now();

    let mut expression = duct::cmd(&spec.program, &spec.args)
        .unchecked()
        .stdout_capture()
        .stderr_capture();

    if let Some(cwd) = &spec.cwd {
        expression = expression.dir(cwd);
    }

    let handle = expression
        .start()
        .map_err(|e| NinetyNineError::RunnerExecution {
            message: format!("failed to spawn {}: {e}", spec.program.display()),
        })?;

    let deadline = Instant::now() + timeout;
    let poll_interval = Duration::from_millis(50);

    loop {
        match handle.try_wait() {
            Ok(Some(output)) => {
                let duration = start.elapsed();
                return Ok(ProcessOutcome {
                    status: output.status,
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                    duration,
                    timed_out: false,
                });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    if let Err(e) = handle.kill() {
                        tracing::warn!("failed to kill process: {e}");
                    }
                    let duration = start.elapsed();
                    // Drain any remaining output after kill.
                    let (stdout, stderr, status) = match handle.wait() {
                        Ok(output) => (
                            String::from_utf8_lossy(&output.stdout).into_owned(),
                            String::from_utf8_lossy(&output.stderr).into_owned(),
                            output.status,
                        ),
                        Err(_) => (String::new(), String::new(), dummy_failure_status()),
                    };
                    return Ok(ProcessOutcome {
                        status,
                        stdout,
                        stderr,
                        duration,
                        timed_out: true,
                    });
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                return Err(NinetyNineError::RunnerExecution {
                    message: format!("error waiting for process: {e}"),
                });
            }
        }
    }
}

#[cfg(unix)]
fn dummy_failure_status() -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(1 << 8)
}

#[cfg(not(unix))]
fn dummy_failure_status() -> ExitStatus {
    std::process::Command::new("false")
        .status()
        .expect("false command")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_timed_success_true() {
        let program = which::which("true").expect("true on PATH");
        let out = run_timed(
            &CommandSpec {
                program,
                args: vec![],
                cwd: None,
            },
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!out.timed_out);
        assert!(out.status.success());
    }

    #[test]
    fn run_timed_timeout_sleep() {
        let program = which::which("sleep").expect("sleep on PATH");
        let out = run_timed(
            &CommandSpec {
                program,
                args: vec!["5".into()],
                cwd: None,
            },
            Duration::from_millis(200),
        )
        .unwrap();
        assert!(out.timed_out);
    }
}
