use std::path::{Path, PathBuf};

use cargo_metadata::{Message, TargetKind};

use crate::error::NinetyNineError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BinaryKind {
    Lib,
    Bin,
    Test,
    Example,
}

#[derive(Debug, Clone)]
pub struct TestBinary {
    pub path: PathBuf,
    pub package_name: String,
    pub binary_name: String,
    pub kind: BinaryKind,
}

pub fn discover_test_binaries(project_root: &Path) -> Result<Vec<TestBinary>, NinetyNineError> {
    let output = std::process::Command::new("cargo")
        .args([
            "test",
            "--no-run",
            "--message-format",
            "json-render-diagnostics",
        ])
        .current_dir(project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| NinetyNineError::BinaryDiscovery {
            message: format!("failed to spawn cargo: {e}"),
        })?
        .wait_with_output()
        .map_err(|e| NinetyNineError::BinaryDiscovery {
            message: format!("failed to wait for cargo: {e}"),
        })?;

    if !output.status.success() {
        return Err(NinetyNineError::BinaryDiscovery {
            message: format!("cargo test --no-run failed with status {}", output.status),
        });
    }

    parse_cargo_messages(&output.stdout)
}

fn parse_cargo_messages(json_bytes: &[u8]) -> Result<Vec<TestBinary>, NinetyNineError> {
    let mut binaries = Vec::new();

    for message in Message::parse_stream(json_bytes) {
        let message = message.map_err(|e| NinetyNineError::BinaryDiscovery {
            message: format!("failed to parse cargo message: {e}"),
        })?;

        if let Message::CompilerArtifact(artifact) = message {
            let is_test = artifact.profile.test;
            if !is_test {
                continue;
            }

            let executable = match artifact.executable {
                Some(ref exe) => PathBuf::from(exe.as_std_path()),
                None => continue,
            };

            let kind = classify_artifact_kind(&artifact.target.kind);

            binaries.push(TestBinary {
                path: executable,
                package_name: artifact.package_id.repr.clone(), // clone: needed to extract from parsed artifact
                binary_name: artifact.target.name.clone(), // clone: needed to extract from parsed artifact
                kind,
            });
        }
    }

    Ok(binaries)
}

fn classify_artifact_kind(kinds: &[TargetKind]) -> BinaryKind {
    for k in kinds {
        match k {
            TargetKind::Test => return BinaryKind::Test,
            TargetKind::Example => return BinaryKind::Example,
            TargetKind::Bin => return BinaryKind::Bin,
            TargetKind::Lib | TargetKind::RLib | TargetKind::DyLib | TargetKind::ProcMacro => {
                return BinaryKind::Lib;
            }
            _ => {}
        }
    }
    BinaryKind::Lib
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case(&[TargetKind::Test], BinaryKind::Test)]
    #[case(&[TargetKind::Example], BinaryKind::Example)]
    #[case(&[TargetKind::Bin], BinaryKind::Bin)]
    #[case(&[TargetKind::Lib], BinaryKind::Lib)]
    #[case(&[TargetKind::RLib], BinaryKind::Lib)]
    #[case(&[TargetKind::ProcMacro], BinaryKind::Lib)]
    fn classifies_binary_kinds(#[case] kinds: &[TargetKind], #[case] expected: BinaryKind) {
        assert_eq!(classify_artifact_kind(kinds), expected);
    }

    proptest! {
        #[test]
        fn parse_cargo_messages_never_panics_on_arbitrary_bytes(
            data in proptest::collection::vec(any::<u8>(), 0..512)
        ) {
            let _ = parse_cargo_messages(&data);
        }
    }
}
