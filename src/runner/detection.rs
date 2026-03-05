use which::which;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailableRunner {
    Nextest,
    CargoTest,
}

impl std::fmt::Display for AvailableRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nextest => write!(f, "cargo-nextest"),
            Self::CargoTest => write!(f, "cargo test"),
        }
    }
}

pub fn detect_available_runner() -> Option<AvailableRunner> {
    if is_nextest_available() {
        Some(AvailableRunner::Nextest)
    } else if is_cargo_available() {
        Some(AvailableRunner::CargoTest)
    } else {
        None
    }
}

pub fn is_nextest_available() -> bool {
    which("cargo-nextest").is_ok()
}

pub fn is_cargo_available() -> bool {
    which("cargo").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_some_runner() {
        let runner = detect_available_runner();
        assert!(
            runner.is_some(),
            "at least cargo should be available in dev environment"
        );
    }

    #[test]
    fn cargo_is_available_in_dev() {
        assert!(is_cargo_available());
    }
}
