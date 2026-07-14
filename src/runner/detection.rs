use which::which;

/// Returns whether `cargo` is on PATH. The native runner builds test
/// binaries via `cargo test --no-run` and executes them directly, so cargo
/// is the only external tool required.
#[must_use]
pub fn cargo_available() -> bool {
    which("cargo").is_ok()
}
