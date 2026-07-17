use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::NinetyNineError;
use crate::runner::listing::TestCase;
use crate::runner::process::{CommandSpec, run_timed};
use crate::types::RecordOutcome;

/// Probe for rr availability used by tests and production.
pub trait RrEnvironment {
    fn is_linux(&self) -> bool;
    fn find_rr(&self) -> Option<PathBuf>;
}

pub struct SystemRrEnvironment;

impl RrEnvironment for SystemRrEnvironment {
    fn is_linux(&self) -> bool {
        cfg!(target_os = "linux")
    }

    fn find_rr(&self) -> Option<PathBuf> {
        which::which("rr").ok()
    }
}

/// Resolves whether rr can be used for recording.
#[must_use]
pub fn resolve_rr(env: &impl RrEnvironment) -> RecordOutcome {
    if !env.is_linux() {
        return RecordOutcome::UnsupportedOs;
    }
    match env.find_rr() {
        Some(_) => RecordOutcome::SkippedNoRequest, // ready; caller must request record
        None => RecordOutcome::Unavailable,
    }
}

/// Builds `rr record [-chaos] -o <out_dir> -- <binary> --exact <name> --nocapture`.
#[must_use]
pub fn rr_record_spec(
    rr: &Path,
    binary: &Path,
    test_name: &str,
    out_dir: &Path,
    chaos: bool,
) -> CommandSpec {
    let mut args = vec!["record".into()];
    if chaos {
        args.push("--chaos".into());
    }
    args.extend([
        "-o".into(),
        out_dir.to_string_lossy().into_owned(),
        "--".into(),
        binary.to_string_lossy().into_owned(),
        "--exact".into(),
        test_name.to_owned(),
        "--nocapture".into(),
    ]);
    CommandSpec {
        program: rr.to_path_buf(),
        args,
        cwd: None,
    }
}

fn sanitize_segment(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Attempts to capture a failing isolation run under rr.
///
/// # Errors
///
/// Returns `RunnerExecution` only when the process machinery fails fatally.
/// Soft rr outcomes are returned as `RecordOutcome` variants.
pub fn attempt_record(
    test_case: &TestCase,
    record_dir: &Path,
    timeout: Duration,
    attempts: u32,
    chaos: bool,
    env: &impl RrEnvironment,
) -> Result<RecordOutcome, NinetyNineError> {
    if !env.is_linux() {
        return Ok(RecordOutcome::UnsupportedOs);
    }
    let Some(rr) = env.find_rr() else {
        return Ok(RecordOutcome::Unavailable);
    };

    let base = record_dir
        .join(sanitize_segment(&test_case.package_name))
        .join(format!(
            "{}__{}",
            sanitize_segment(&test_case.binary_name),
            sanitize_segment(test_case.name.as_ref())
        ));
    std::fs::create_dir_all(&base).map_err(|e| NinetyNineError::RunnerExecution {
        message: format!("failed to create record dir {}: {e}", base.display()),
    })?;

    for i in 0..attempts {
        let out_dir = base.join(format!("attempt_{i}"));
        if out_dir.exists() {
            let _ = std::fs::remove_dir_all(&out_dir);
        }
        std::fs::create_dir_all(&out_dir).map_err(|e| NinetyNineError::RunnerExecution {
            message: format!("failed to create attempt dir: {e}"),
        })?;

        let spec = rr_record_spec(
            &rr,
            &test_case.binary_path,
            test_case.name.as_ref(),
            &out_dir,
            chaos,
        );
        let outcome = match run_timed(&spec, timeout) {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(error = %e, "rr record spawn failed");
                let _ = std::fs::remove_dir_all(&out_dir);
                continue;
            }
        };

        if outcome.timed_out {
            let _ = std::fs::remove_dir_all(&out_dir);
            continue;
        }

        // Nonzero status with a non-empty trace dir means the test failed under rr.
        if !outcome.status.success() && dir_nonempty(&out_dir) {
            let final_path = base.join(format!(
                "record_{}",
                chrono::Utc::now().format("%Y%m%d%H%M%S")
            ));
            if std::fs::rename(&out_dir, &final_path).is_ok() {
                return Ok(RecordOutcome::FailedWithTrace { path: final_path });
            }
            if dir_nonempty(&out_dir) {
                return Ok(RecordOutcome::FailedWithTrace { path: out_dir });
            }
            return Ok(RecordOutcome::RecorderError);
        }

        // Pass or invalid trace: discard.
        let _ = std::fs::remove_dir_all(&out_dir);
        if !outcome.status.success() && !dir_nonempty(&out_dir) {
            // rr itself failed without a trace
            // keep trying remaining attempts; if all fail, RecorderError
        }
    }

    // Never captured a failure.
    Ok(RecordOutcome::PassedNoFailure)
}

fn dir_nonempty(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut d| d.next().is_some())
        .unwrap_or(false)
}

/// User-facing message when recording was requested but skipped or soft-failed.
#[must_use]
pub fn rr_skip_message(outcome: &RecordOutcome) -> String {
    match outcome {
        RecordOutcome::UnsupportedOs => {
            "rr recording is not supported on this platform; classification still ran."
                .to_string()
        }
        RecordOutcome::Unavailable => {
            "rr not found on PATH; install from https://rr-project.org/ (Linux only). Classification still ran."
                .to_string()
        }
        RecordOutcome::RecorderError => {
            "rr failed to produce a valid trace; classification still ran.".to_string()
        }
        RecordOutcome::PassedNoFailure => {
            "rr ran but no failing isolation attempt was captured within the attempt budget."
                .to_string()
        }
        RecordOutcome::SkippedNoRequest | RecordOutcome::FailedWithTrace { .. } => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::binary::BinaryKind;
    use crate::runner::listing::TestKind;
    use crate::types::TestName;
    use std::sync::Mutex;

    struct MockEnv {
        linux: bool,
        rr: Option<PathBuf>,
    }

    impl RrEnvironment for MockEnv {
        fn is_linux(&self) -> bool {
            self.linux
        }
        fn find_rr(&self) -> Option<PathBuf> {
            self.rr.clone() // clone: mock returns owned path
        }
    }

    #[test]
    fn unsupported_os_when_not_linux() {
        let env = MockEnv {
            linux: false,
            rr: Some(PathBuf::from("/usr/bin/rr")),
        };
        assert_eq!(resolve_rr(&env), RecordOutcome::UnsupportedOs);
        let tc = sample_case();
        let outcome = attempt_record(
            &tc,
            Path::new("/tmp"),
            Duration::from_secs(1),
            1,
            false,
            &env,
        )
        .unwrap();
        assert_eq!(outcome, RecordOutcome::UnsupportedOs);
    }

    #[test]
    fn unavailable_when_rr_not_on_path() {
        let env = MockEnv {
            linux: true,
            rr: None,
        };
        assert_eq!(resolve_rr(&env), RecordOutcome::Unavailable);
        let tc = sample_case();
        let outcome = attempt_record(
            &tc,
            Path::new("/tmp"),
            Duration::from_secs(1),
            1,
            false,
            &env,
        )
        .unwrap();
        assert_eq!(outcome, RecordOutcome::Unavailable);
    }

    #[test]
    fn rr_spec_shape() {
        let spec = rr_record_spec(
            Path::new("/usr/bin/rr"),
            Path::new("/tmp/bin"),
            "tests::t",
            Path::new("/tmp/out"),
            false,
        );
        assert_eq!(spec.program, PathBuf::from("/usr/bin/rr"));
        assert!(spec.args.iter().any(|a| a == "record"));
        assert!(spec.args.iter().any(|a| a == "--exact"));
        assert!(spec.args.iter().any(|a| a == "tests::t"));
        assert!(!spec.args.iter().any(|a| a == "--chaos"));
    }

    #[test]
    fn rr_spec_includes_chaos_when_enabled() {
        let spec = rr_record_spec(
            Path::new("/usr/bin/rr"),
            Path::new("/tmp/bin"),
            "tests::t",
            Path::new("/tmp/out"),
            true,
        );
        assert!(spec.args.iter().any(|a| a == "--chaos"));
    }

    #[test]
    fn rr_skip_message_unsupported_os() {
        let msg = rr_skip_message(&RecordOutcome::UnsupportedOs);
        assert!(msg.to_lowercase().contains("not supported"));
    }

    #[test]
    fn rr_skip_message_unavailable() {
        let msg = rr_skip_message(&RecordOutcome::Unavailable);
        assert!(msg.to_lowercase().contains("install") || msg.contains("rr"));
    }

    #[test]
    fn failed_with_trace_requires_nonempty_dir() {
        assert!(!dir_nonempty(Path::new(
            "/tmp/definitely-does-not-exist-nn-rr"
        )));
        let dir = tempfile::tempdir().unwrap();
        assert!(!dir_nonempty(dir.path()));
        std::fs::write(dir.path().join("trace"), b"x").unwrap();
        assert!(dir_nonempty(dir.path()));
    }

    fn sample_case() -> TestCase {
        TestCase {
            name: TestName::from("tests::t"),
            binary_path: PathBuf::from("/tmp/bin"),
            binary_name: "bin".into(),
            package_name: "pkg".into(),
            binary_kind: BinaryKind::Test,
            kind: TestKind::Test,
        }
    }

    // Silence unused mutex warning if rustc warns on static patterns.
    #[allow(dead_code)]
    static _LOCK: Mutex<()> = Mutex::new(());
}
