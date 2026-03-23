use crate::types::TestEnvironment;

const CI_PROVIDERS: &[(&str, &str)] = &[
    ("GITHUB_ACTIONS", "GitHub Actions"),
    ("GITLAB_CI", "GitLab CI"),
    ("JENKINS_URL", "Jenkins"),
    ("CIRCLECI", "CircleCI"),
    ("BUILDKITE", "Buildkite"),
    ("TF_BUILD", "Azure DevOps"),
];

#[must_use]
pub fn detect_git_info() -> (String, String) {
    let commit = duct::cmd!("git", "rev-parse", "HEAD")
        .stdout_capture()
        .stderr_null()
        .read()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let branch = duct::cmd!("git", "branch", "--show-current")
        .stdout_capture()
        .stderr_null()
        .read()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    (commit, branch)
}

#[must_use]
pub fn detect_environment() -> TestEnvironment {
    let rust_version = duct::cmd!("rustc", "--version")
        .stdout_capture()
        .stderr_null()
        .read()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    TestEnvironment {
        os: std::env::consts::OS.to_string(),
        rust_version,
        cpu_count: std::thread::available_parallelism()
            .map(|n| u32::try_from(n.get()).unwrap_or(u32::MAX))
            .unwrap_or(1),
        memory_gb: detect_memory_gb(),
        is_ci: std::env::var("CI").is_ok(),
        ci_provider: detect_ci_provider(),
    }
}

fn detect_memory_gb() -> f64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|line| line.starts_with("MemTotal:"))
                .and_then(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .and_then(|kb| kb.parse::<u64>().ok())
                })
                .map(|kb| f64::from(u32::try_from(kb / 1024).unwrap_or(u32::MAX)) / 1024.0)
        })
        .unwrap_or(0.0)
}

fn detect_ci_provider() -> Option<String> {
    CI_PROVIDERS
        .iter()
        .find(|(env_var, _)| std::env::var(env_var).is_ok())
        .map(|(_, name)| (*name).to_string())
}
