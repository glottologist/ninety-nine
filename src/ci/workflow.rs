use std::fmt::Write;

use crate::config::model::Config;

#[must_use]
pub fn generate_github_actions(config: &Config) -> String {
    let mut yaml = String::with_capacity(2048);

    writeln!(yaml, "name: Flaky Test Detection").ok();
    writeln!(yaml, "on:").ok();
    writeln!(yaml, "  schedule:").ok();
    writeln!(yaml, "    - cron: '0 3 * * 1'").ok();
    writeln!(yaml, "  workflow_dispatch:").ok();
    writeln!(yaml).ok();
    writeln!(yaml, "jobs:").ok();
    writeln!(yaml, "  flaky-tests:").ok();
    writeln!(yaml, "    runs-on: ubuntu-latest").ok();
    writeln!(yaml, "    steps:").ok();
    writeln!(yaml, "      - uses: actions/checkout@v4").ok();
    writeln!(yaml).ok();
    writeln!(yaml, "      - name: Install Rust").ok();
    writeln!(yaml, "        uses: dtolnay/rust-toolchain@stable").ok();
    writeln!(yaml).ok();
    writeln!(yaml, "      - name: Install cargo-nextest").ok();
    writeln!(yaml, "        uses: taiki-e/install-action@nextest").ok();
    writeln!(yaml).ok();
    writeln!(yaml, "      - name: Install cargo-ninety-nine").ok();
    writeln!(yaml, "        run: cargo install cargo-ninety-nine").ok();
    writeln!(yaml).ok();
    writeln!(yaml, "      - name: Run flaky test detection").ok();

    let mut run_cmd = String::from("cargo ninety-nine test");
    write!(run_cmd, " -n {}", config.detection.min_runs).ok();
    write!(
        run_cmd,
        " --confidence {}",
        config.detection.confidence_threshold
    )
    .ok();

    writeln!(yaml, "        run: {run_cmd}").ok();
    writeln!(yaml, "        continue-on-error: true").ok();

    writeln!(yaml).ok();
    writeln!(yaml, "      - name: Export results").ok();
    writeln!(yaml, "        if: always()").ok();
    writeln!(
        yaml,
        "        run: cargo ninety-nine export junit flaky-results.xml"
    )
    .ok();
    writeln!(yaml).ok();
    writeln!(yaml, "      - name: Upload test results").ok();
    writeln!(yaml, "        if: always()").ok();
    writeln!(yaml, "        uses: actions/upload-artifact@v4").ok();
    writeln!(yaml, "        with:").ok();
    writeln!(yaml, "          name: flaky-test-results").ok();
    writeln!(yaml, "          path: flaky-results.xml").ok();

    yaml
}

#[must_use]
pub fn generate_gitlab_ci(config: &Config) -> String {
    let mut yaml = String::with_capacity(1024);

    writeln!(yaml, "flaky-test-detection:").ok();
    writeln!(yaml, "  stage: test").ok();
    writeln!(yaml, "  image: rust:latest").ok();
    writeln!(yaml, "  rules:").ok();
    writeln!(yaml, "    - if: $CI_PIPELINE_SOURCE == \"schedule\"").ok();
    writeln!(yaml, "    - if: $CI_PIPELINE_SOURCE == \"web\"").ok();
    writeln!(yaml, "  before_script:").ok();
    writeln!(yaml, "    - cargo install cargo-nextest cargo-ninety-nine").ok();
    writeln!(yaml, "  script:").ok();

    let mut run_cmd = String::from("    - cargo ninety-nine test");
    write!(run_cmd, " -n {}", config.detection.min_runs).ok();
    write!(
        run_cmd,
        " --confidence {}",
        config.detection.confidence_threshold
    )
    .ok();

    writeln!(yaml, "{run_cmd}").ok();
    writeln!(
        yaml,
        "    - cargo ninety-nine export junit flaky-results.xml"
    )
    .ok();

    writeln!(yaml, "  allow_failure: true").ok();

    writeln!(yaml, "  artifacts:").ok();
    writeln!(yaml, "    when: always").ok();
    writeln!(yaml, "    reports:").ok();
    writeln!(yaml, "      junit: flaky-results.xml").ok();

    yaml
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::Config;
    use rstest::rstest;

    #[rstest]
    #[case("name: Flaky Test Detection")]
    #[case("cargo install cargo-ninety-nine")]
    #[case("cargo ninety-nine test")]
    #[case("actions/upload-artifact")]
    #[case("continue-on-error: true")]
    fn github_actions_default_config_contains(#[case] expected: &str) {
        let yaml = generate_github_actions(&Config::default());
        assert!(yaml.contains(expected), "missing: {expected}");
    }

    #[rstest]
    #[case("flaky-test-detection:")]
    #[case("cargo install cargo-nextest cargo-ninety-nine")]
    #[case("cargo ninety-nine test")]
    #[case("junit: flaky-results.xml")]
    #[case("allow_failure: true")]
    fn gitlab_ci_default_config_contains(#[case] expected: &str) {
        let yaml = generate_gitlab_ci(&Config::default());
        assert!(yaml.contains(expected), "missing: {expected}");
    }

    #[rstest]
    #[case(25, 0.99, "-n 25", "--confidence 0.99")]
    #[case(15, 0.9, "-n 15", "--confidence 0.9")]
    fn github_actions_uses_config_values(
        #[case] min_runs: u32,
        #[case] confidence: f64,
        #[case] expected_runs: &str,
        #[case] expected_conf: &str,
    ) {
        let mut config = Config::default();
        config.detection.min_runs = min_runs;
        config.detection.confidence_threshold = confidence;
        let yaml = generate_github_actions(&config);
        assert!(yaml.contains(expected_runs));
        assert!(yaml.contains(expected_conf));
    }

    #[rstest]
    #[case(15, 0.9, "-n 15", "--confidence 0.9")]
    #[case(25, 0.99, "-n 25", "--confidence 0.99")]
    fn gitlab_ci_uses_config_values(
        #[case] min_runs: u32,
        #[case] confidence: f64,
        #[case] expected_runs: &str,
        #[case] expected_conf: &str,
    ) {
        let mut config = Config::default();
        config.detection.min_runs = min_runs;
        config.detection.confidence_threshold = confidence;
        let yaml = generate_gitlab_ci(&config);
        assert!(yaml.contains(expected_runs));
        assert!(yaml.contains(expected_conf));
    }
}
