use std::path::{Path, PathBuf};

use cargo_metadata::{Message, TargetKind};

use crate::error::NinetyNineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Discovers test binaries in the given project by running `cargo test --no-run`.
///
/// # Errors
///
/// Returns `BinaryDiscovery` if cargo cannot be spawned, exits with a failure
/// status, or produces unparseable output.
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
                package_name: package_name_from_id(&artifact.package_id.repr),
                binary_name: artifact.target.name.clone(), // clone: needed to extract from parsed artifact
                kind,
            });
        }
    }

    Ok(binaries)
}

/// Extracts the bare package name from a cargo package-ID.
///
/// Handles both the pre-1.77 form `name version (source)` and the spec form
/// `source#name@version`, where the fragment collapses to a bare version
/// when the name equals the last path segment of the source URL.
fn package_name_from_id(repr: &str) -> String {
    if let Some((name, rest)) = repr.split_once(' ') {
        if rest.contains('(') {
            return name.to_string();
        }
    }

    match repr.rsplit_once('#') {
        Some((url, fragment)) => match fragment.rsplit_once('@') {
            Some((name, _version)) => name.to_string(),
            None => url.rsplit('/').next().unwrap_or(repr).to_string(),
        },
        None => repr.to_string(),
    }
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

    #[rstest]
    #[case("path+file:///home/user/proj#0.3.4", "proj")]
    #[case("path+file:///home/user/proj#alt-name@0.3.4", "alt-name")]
    #[case(
        "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.219",
        "serde"
    )]
    #[case(
        "cargo-ninety-nine 0.3.4 (path+file:///home/user/proj)",
        "cargo-ninety-nine"
    )]
    fn package_name_extracted_from_all_id_formats(#[case] repr: &str, #[case] expected: &str) {
        assert_eq!(package_name_from_id(repr), expected);
    }

    proptest! {
        #[test]
        fn parse_cargo_messages_never_panics_on_arbitrary_bytes(
            data in proptest::collection::vec(any::<u8>(), 0..512)
        ) {
            let _ = parse_cargo_messages(&data);
        }

        #[test]
        fn package_name_from_id_never_panics(repr in ".{0,120}") {
            let _ = package_name_from_id(&repr);
        }
    }
}
