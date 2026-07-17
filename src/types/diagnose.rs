use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::types::TestName;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestId {
    pub package_name: String,
    pub binary_name: String,
    pub test_name: TestName,
}

impl TestId {
    #[must_use]
    pub fn key(&self) -> String {
        format!(
            "{}::{}::{}",
            self.package_name, self.binary_name, self.test_name
        )
    }

    #[must_use]
    pub fn new(
        package_name: impl Into<String>,
        binary_name: impl Into<String>,
        test_name: impl Into<TestName>,
    ) -> Self {
        Self {
            package_name: package_name.into(),
            binary_name: binary_name.into(),
            test_name: test_name.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlakeClass {
    Broken,
    Intrinsic,
    Contention,
}

impl FlakeClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Broken => "broken",
            Self::Intrinsic => "intrinsic",
            Self::Contention => "contention",
        }
    }
}

impl std::fmt::Display for FlakeClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FlakeClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "broken" => Ok(Self::Broken),
            "intrinsic" => Ok(Self::Intrinsic),
            "contention" => Ok(Self::Contention),
            _ => Err(format!("unknown flake class: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseCounts {
    pub stress_runs: u32,
    pub stress_failures: u32,
    pub isolation_runs: u32,
    pub isolation_failures: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordOutcome {
    SkippedNoRequest,
    Unavailable,
    UnsupportedOs,
    RecorderError,
    PassedNoFailure,
    FailedWithTrace { path: PathBuf },
}

impl RecordOutcome {
    #[must_use]
    pub fn recording_path(&self) -> Option<&std::path::Path> {
        match self {
            Self::FailedWithTrace { path } => Some(path.as_path()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResult {
    pub test_id: TestId,
    pub class: FlakeClass,
    pub counts: PhaseCounts,
    pub recording: RecordOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    Isolation,
}

impl RunPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Isolation => "isolation",
        }
    }
}

impl FromStr for RunPhase {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "isolation" => Ok(Self::Isolation),
            _ => Err(format!("unknown run phase: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Detection,
    Diagnose,
}

impl SessionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Detection => "detection",
            Self::Diagnose => "diagnose",
        }
    }
}

impl FromStr for SessionKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "detection" => Ok(Self::Detection),
            "diagnose" => Ok(Self::Diagnose),
            _ => Err(format!("unknown session kind: {s}")),
        }
    }
}

impl Default for SessionKind {
    fn default() -> Self {
        Self::Detection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_key_format() {
        let id = TestId {
            package_name: "pkg".into(),
            binary_name: "bin".into(),
            test_name: TestName::from("mod::t"),
        };
        assert_eq!(id.key(), "pkg::bin::mod::t");
    }

    #[test]
    fn flake_class_roundtrip_str() {
        for c in [
            FlakeClass::Broken,
            FlakeClass::Intrinsic,
            FlakeClass::Contention,
        ] {
            assert_eq!(c.as_str().parse::<FlakeClass>().unwrap(), c);
        }
    }

    #[test]
    fn session_kind_and_run_phase_roundtrip() {
        assert_eq!(
            SessionKind::Diagnose
                .as_str()
                .parse::<SessionKind>()
                .unwrap(),
            SessionKind::Diagnose
        );
        assert_eq!(
            RunPhase::Isolation.as_str().parse::<RunPhase>().unwrap(),
            RunPhase::Isolation
        );
    }
}
