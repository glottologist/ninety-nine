use std::fmt::Write;

use crate::config::model::Config;

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
    writeln!(yaml, "      - name: Detect flaky tests").ok();

    let mut run_cmd = String::from("cargo ninety-nine detect");
    write!(run_cmd, " -n {}", config.detection.min_runs).ok();
    write!(
        run_cmd,
        " --confidence {}",
        config.detection.confidence_threshold
    )
    .ok();

    if config.ci.fail_on_flaky {
        writeln!(yaml, "        run: {run_cmd}").ok();
    } else {
        writeln!(yaml, "        run: {run_cmd}").ok();
        writeln!(yaml, "        continue-on-error: true").ok();
    }

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

    let mut run_cmd = String::from("    - cargo ninety-nine detect");
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

    if !config.ci.fail_on_flaky {
        writeln!(yaml, "  allow_failure: true").ok();
    }

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

    #[test]
    fn github_actions_contains_required_sections() {
        let config = Config::default();
        let yaml = generate_github_actions(&config);
        assert!(yaml.contains("name: Flaky Test Detection"));
        assert!(yaml.contains("cargo install cargo-ninety-nine"));
        assert!(yaml.contains("cargo ninety-nine detect"));
        assert!(yaml.contains("actions/upload-artifact"));
    }

    #[test]
    fn github_actions_fail_on_flaky_omits_continue_on_error() {
        let mut config = Config::default();
        config.ci.fail_on_flaky = true;
        let yaml = generate_github_actions(&config);
        assert!(!yaml.contains("continue-on-error"));
    }

    #[test]
    fn github_actions_no_fail_has_continue_on_error() {
        let mut config = Config::default();
        config.ci.fail_on_flaky = false;
        let yaml = generate_github_actions(&config);
        assert!(yaml.contains("continue-on-error: true"));
    }

    #[test]
    fn gitlab_ci_contains_required_sections() {
        let config = Config::default();
        let yaml = generate_gitlab_ci(&config);
        assert!(yaml.contains("flaky-test-detection:"));
        assert!(yaml.contains("cargo install cargo-nextest cargo-ninety-nine"));
        assert!(yaml.contains("cargo ninety-nine detect"));
        assert!(yaml.contains("junit: flaky-results.xml"));
    }

    #[test]
    fn gitlab_ci_fail_on_flaky_omits_allow_failure() {
        let mut config = Config::default();
        config.ci.fail_on_flaky = true;
        let yaml = generate_gitlab_ci(&config);
        assert!(!yaml.contains("allow_failure"));
    }

    #[test]
    fn gitlab_ci_no_fail_has_allow_failure() {
        let mut config = Config::default();
        config.ci.fail_on_flaky = false;
        let yaml = generate_gitlab_ci(&config);
        assert!(yaml.contains("allow_failure: true"));
    }

    #[test]
    fn github_actions_uses_config_values() {
        let mut config = Config::default();
        config.detection.min_runs = 25;
        config.detection.confidence_threshold = 0.99;
        let yaml = generate_github_actions(&config);
        assert!(yaml.contains("-n 25"));
        assert!(yaml.contains("--confidence 0.99"));
    }

    #[test]
    fn gitlab_ci_uses_config_values() {
        let mut config = Config::default();
        config.detection.min_runs = 15;
        config.detection.confidence_threshold = 0.9;
        let yaml = generate_gitlab_ci(&config);
        assert!(yaml.contains("-n 15"));
        assert!(yaml.contains("--confidence 0.9"));
    }
}
