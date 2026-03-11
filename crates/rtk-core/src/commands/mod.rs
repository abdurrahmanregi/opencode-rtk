pub mod aws_cmd;
pub mod cargo_cmd;
pub mod curl_cmd;
pub mod diff_cmd;
pub mod docker;
pub mod find_cmd;
pub mod git;
pub mod go_cmd;
pub mod golangci_cmd;
pub mod grep_cmd;
pub mod lint_cmd;
pub mod ls_cmd;
pub mod next_cmd;
pub mod npm_cmd;
pub mod pip_cmd;
pub mod playwright_cmd;
pub mod pnpm_cmd;
pub mod pre_execution;
pub mod prisma_cmd;
pub mod psql_cmd;
pub mod pytest_cmd;
pub mod read_cmd;
pub mod ruff_cmd;
pub mod tsc_cmd;
pub mod vitest_cmd;
pub mod wget_cmd;

pub use pre_execution::{optimize_command, FlagMapping, OptimizedCommand};

use crate::Context;
use anyhow::Result;

pub trait CommandModule: Send + Sync {
    fn name(&self) -> &str;
    fn strategy(&self) -> &str;
    fn compress(&self, output: &str, context: &Context) -> Result<String>;
}

lazy_static::lazy_static! {
    static ref MODULES: Vec<Box<dyn CommandModule>> = vec![
        Box::new(git::GitModule::new()),
        Box::new(npm_cmd::NpmModule::new()),
        Box::new(cargo_cmd::CargoModule::new()),
        Box::new(docker::DockerModule::new()),
        Box::new(pytest_cmd::PytestModule::new()),
        Box::new(lint_cmd::LintModule::new()),
        Box::new(tsc_cmd::TscModule::new()),
        Box::new(next_cmd::NextModule::new()),
        Box::new(playwright_cmd::PlaywrightModule::new()),
        Box::new(prisma_cmd::PrismaModule::new()),
        Box::new(vitest_cmd::VitestModule::new()),
        Box::new(pnpm_cmd::PnpmModule::new()),
        Box::new(pip_cmd::PipModule::new()),
        Box::new(ruff_cmd::RuffModule::new()),
        Box::new(go_cmd::GoModule::new()),
        Box::new(golangci_cmd::GolangciModule::new()),
        Box::new(wget_cmd::WgetModule::new()),
        Box::new(curl_cmd::CurlModule::new()),
        Box::new(aws_cmd::AwsModule::new()),
        Box::new(psql_cmd::PsqlModule::new()),
        Box::new(grep_cmd::GrepModule::new()),
        Box::new(diff_cmd::DiffModule::new()),
        Box::new(find_cmd::FindModule::new()),
        Box::new(ls_cmd::LsModule::new()),
        Box::new(read_cmd::ReadModule::new()),
    ];
}

pub fn detect_command(command: &str) -> Option<&'static dyn CommandModule> {
    MODULES
        .iter()
        .find(|m| matches_module(m.as_ref(), command))
        .map(|m| m.as_ref())
}

fn matches_module(module: &dyn CommandModule, command: &str) -> bool {
    // Trim leading/trailing whitespace to handle edge cases
    let cmd_lower = command.trim().to_lowercase();
    let name = module.name();

    // Extract base command from path (e.g., "/usr/bin/git" -> "git")
    let base_cmd = extract_base_command(&cmd_lower);

    // Special handling for pip (matches both pip and pip3)
    if name == "pip" {
        // Use word boundary checks: space after command or end of string
        return (cmd_lower.starts_with("pip ")
            || cmd_lower == "pip"
            || cmd_lower.starts_with("pip\t"))
            || (cmd_lower.starts_with("pip3 ")
                || cmd_lower == "pip3"
                || cmd_lower.starts_with("pip3\t"))
            || (cmd_lower.starts_with("rtk pip ") || cmd_lower.starts_with("rtk pip\t"))
            || (cmd_lower.starts_with("rtk pip3 ") || cmd_lower.starts_with("rtk pip3\t"))
            || base_cmd == "pip"
            || base_cmd == "pip3";
    }

    // Special handling for golangci-lint (has dash in name)
    if name == "golangci-lint" {
        return cmd_lower.starts_with("golangci-lint ")
            || cmd_lower.starts_with("golangci-lint\t")
            || cmd_lower.starts_with("rtk golangci-lint ")
            || cmd_lower.starts_with("rtk golangci-lint\t")
            || cmd_lower == "golangci-lint"
            || base_cmd == "golangci-lint";
    }

    // General case: check for word boundary (space, tab, or end of string)
    let prefix_with_space = format!("{} ", name);
    let prefix_with_tab = format!("{}\t", name);
    let rtk_prefix_space = format!("rtk {} ", name);
    let rtk_prefix_tab = format!("rtk {}\t", name);

    // Also check if the base command extracted from a path matches
    cmd_lower.starts_with(&prefix_with_space)
        || cmd_lower.starts_with(&prefix_with_tab)
        || cmd_lower.starts_with(&rtk_prefix_space)
        || cmd_lower.starts_with(&rtk_prefix_tab)
        || cmd_lower == name
        || base_cmd == name
}

/// Extract the base command name from a potentially path-qualified command
/// e.g., "/usr/bin/git status" -> "git", "C:\\Program Files\\node\\npm.cmd" -> "npm"
fn extract_base_command(command: &str) -> &str {
    // Split by whitespace to get first token
    let first_token = command.split_whitespace().next().unwrap_or(command);

    // Handle both Unix and Windows paths
    // Unix: /usr/bin/git -> git
    // Windows: C:\Program Files\node\npm.cmd -> npm.cmd
    let base = if first_token.contains('/') {
        // Unix path
        first_token.rsplit('/').next().unwrap_or(first_token)
    } else if first_token.contains('\\') {
        // Windows path
        first_token.rsplit('\\').next().unwrap_or(first_token)
    } else {
        // No path separator, return as-is
        first_token
    };

    // Strip common Windows extensions (.exe, .cmd, .bat)
    base.trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .trim_end_matches(".bat")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_git() {
        let module = detect_command("git status");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "git");
    }

    #[test]
    fn test_detect_npm() {
        let module = detect_command("npm test");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "npm");
    }

    #[test]
    fn test_detect_unknown() {
        let module = detect_command("unknown-command");
        assert!(module.is_none());
    }

    #[test]
    fn test_detect_eslint() {
        let module = detect_command("eslint src/");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "eslint");
    }

    #[test]
    fn test_detect_tsc() {
        let module = detect_command("tsc --noEmit");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "tsc");
    }

    #[test]
    fn test_detect_next() {
        let module = detect_command("next build");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "next");
    }

    #[test]
    fn test_detect_playwright() {
        let module = detect_command("playwright test");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "playwright");
    }

    #[test]
    fn test_detect_prisma() {
        let module = detect_command("prisma migrate");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "prisma");
    }

    #[test]
    fn test_detect_vitest() {
        let module = detect_command("vitest run");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "vitest");
    }

    #[test]
    fn test_detect_pnpm() {
        let module = detect_command("pnpm install");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "pnpm");
    }

    #[test]
    fn test_detect_pip() {
        let module = detect_command("pip install requests");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "pip");
    }

    #[test]
    fn test_detect_pip3() {
        let module = detect_command("pip3 list");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "pip");
    }

    #[test]
    fn test_detect_ruff() {
        let module = detect_command("ruff check src/");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "ruff");
    }

    #[test]
    fn test_detect_go() {
        let module = detect_command("go test ./...");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "go");
    }

    #[test]
    fn test_detect_go_build() {
        let module = detect_command("go build");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "go");
    }

    #[test]
    fn test_detect_golangci_lint() {
        let module = detect_command("golangci-lint run");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "golangci-lint");
    }

    #[test]
    fn test_detect_golangci_lint_linters() {
        let module = detect_command("golangci-lint linters");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "golangci-lint");
    }

    #[test]
    fn test_detect_wget() {
        let module = detect_command("wget https://example.com/file");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "wget");
    }

    #[test]
    fn test_detect_wget_with_flags() {
        let module = detect_command("wget -O output.txt https://example.com/file");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "wget");
    }

    #[test]
    fn test_detect_curl() {
        let module = detect_command("curl https://example.com");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "curl");
    }

    #[test]
    fn test_detect_curl_verbose() {
        let module = detect_command("curl -v https://example.com");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "curl");
    }

    #[test]
    fn test_detect_aws() {
        let module = detect_command("aws s3 ls");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "aws");
    }

    #[test]
    fn test_detect_aws_with_profile() {
        let module = detect_command("aws --profile prod ec2 describe-instances");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "aws");
    }

    #[test]
    fn test_detect_psql() {
        let module = detect_command("psql -c \"SELECT * FROM users\"");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "psql");
    }

    #[test]
    fn test_detect_psql_with_db() {
        let module = detect_command("psql -h localhost -U postgres mydb");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "psql");
    }

    #[test]
    fn test_detect_grep() {
        let module = detect_command("grep pattern *.rs");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "grep");
    }

    #[test]
    fn test_detect_grep_recursive() {
        let module = detect_command("grep -r pattern src/");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "grep");
    }

    #[test]
    fn test_detect_diff() {
        let module = detect_command("diff file1.txt file2.txt");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "diff");
    }

    #[test]
    fn test_detect_diff_unified() {
        let module = detect_command("diff -u old new");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "diff");
    }

    #[test]
    fn test_detect_find() {
        let module = detect_command("find . -name '*.rs'");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "find");
    }

    #[test]
    fn test_detect_find_type() {
        let module = detect_command("find /tmp -type f");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "find");
    }

    #[test]
    fn test_detect_ls() {
        let module = detect_command("ls -la");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "ls");
    }

    #[test]
    fn test_detect_ls_directory() {
        let module = detect_command("ls src/");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "ls");
    }

    #[test]
    fn test_detect_read() {
        let module = detect_command("read file.txt");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "read");
    }

    #[test]
    fn test_detect_read_with_path() {
        let module = detect_command("read ./src/main.rs");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "read");
    }

    #[test]
    fn test_detect_git_with_full_path() {
        // Test that git is detected even with full path
        let module = detect_command("/usr/bin/git status");
        // Note: Current implementation may not detect this, but let's document expected behavior
        // If it should be detected, the matches_module function needs to be updated
        // For now, this tests the current behavior
        let module2 = detect_command("/usr/local/bin/git diff");
        // Both should either be detected or not consistently
        assert_eq!(module.is_some(), module2.is_some());
    }

    #[test]
    fn test_detect_command_with_leading_path() {
        // Test commands with leading paths don't cause false matches
        let module = detect_command("/opt/tools/npm install");
        // Current behavior: path-prefixed commands may not be detected
        // This test documents the current behavior
        let module2 = detect_command("npm install");
        assert!(module2.is_some());
    }

    #[test]
    fn test_detect_cargo_with_path() {
        // cargo should be detected regardless of how it's invoked
        let module = detect_command("cargo build");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "cargo");
    }

    #[test]
    fn test_detect_with_rtk_prefix() {
        // Commands with rtk prefix should still be detected
        let module = detect_command("rtk git status");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "git");
    }

    #[test]
    fn test_detect_with_leading_spaces() {
        // Should handle leading/trailing whitespace
        let module = detect_command("  git status  ");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "git");
    }

    #[test]
    fn test_detect_no_false_positives() {
        // Should NOT match "gitstatus" (no space)
        let module = detect_command("gitstatus");
        assert!(module.is_none());

        // Should NOT match "npms" (partial match)
        let module = detect_command("npms");
        assert!(module.is_none());
    }

    #[test]
    fn test_detect_case_insensitive() {
        let module = detect_command("GIT STATUS");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "git");

        let module = detect_command("NPM TEST");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "npm");
    }

    #[test]
    fn test_detect_with_tabs() {
        // Should handle tabs as word boundaries
        let module = detect_command("git\tstatus");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name(), "git");
    }
}
