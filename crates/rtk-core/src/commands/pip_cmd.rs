use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct PipModule {
    error_strategy: ErrorOnly,
    grouping_strategy: GroupingByPattern,
}

impl PipModule {
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorOnly,
            grouping_strategy: GroupingByPattern,
        }
    }

    /// Detect which pip subcommand is being used
    fn detect_subcommand(&self, command: &str) -> Option<String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.len() >= 2 && (parts[0] == "pip" || parts[0] == "pip3") {
            Some(parts[1].to_string())
        } else {
            None
        }
    }

    /// Handle pip list - format as table
    fn handle_list(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no packages)".to_string());
        }

        let lines: Vec<&str> = output.lines().collect();

        // Check if it's the default format (Package Version)
        if lines.len() >= 2 && lines[0].contains("Package") && lines[0].contains("Version") {
            let packages: Vec<(&str, &str)> = lines
                .iter()
                .skip(2) // Skip header and separator
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        Some((parts[0], parts[1]))
                    } else {
                        None
                    }
                })
                .collect();

            if packages.is_empty() {
                return Ok("(no packages)".to_string());
            }

            let count = packages.len();
            let mut result = vec![format!("{} packages installed", count)];

            // Show first 20 packages
            for (name, version) in packages.iter().take(20) {
                result.push(format!("  {}=={}", name, version));
            }

            if count > 20 {
                result.push(format!("  ... and {} more", count - 20));
            }

            return Ok(result.join("\n"));
        }
        // Fallback to grouping for other formats
        self.grouping_strategy.compress(output)
    }

    /// Handle pip list --outdated - show only outdated packages
    fn handle_outdated(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(all packages up to date)".to_string());
        }

        let lines: Vec<&str> = output.lines().collect();

        // Parse outdated packages table
        // Format: Package Version Latest Type
        if lines.len() >= 2 && lines[0].contains("Package") {
            let packages: Vec<(&str, &str, &str)> = lines
                .iter()
                .skip(2) // Skip header and separator
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        Some((parts[0], parts[1], parts[2]))
                    } else {
                        None
                    }
                })
                .collect();

            if packages.is_empty() {
                return Ok("(all packages up to date)".to_string());
            }

            let count = packages.len();
            let mut result = vec![format!("{} outdated packages", count)];

            for (name, current, latest) in packages.iter().take(20) {
                result.push(format!("  {} {} -> {}", name, current, latest));
            }

            if count > 20 {
                result.push(format!("  ... and {} more", count - 20));
            }

            return Ok(result.join("\n"));
        }

        // Fallback
        if output.to_lowercase().contains("outdated") {
            return Ok("(all packages up to date)".to_string());
        }

        self.grouping_strategy.compress(output)
    }

    /// Handle pip install - progress filtering
    fn handle_install(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code != 0 {
            return self.error_strategy.compress(output);
        }

        if output.is_empty() {
            return Ok("(installed)".to_string());
        }

        let mut result = Vec::new();
        let mut installed: Vec<String> = Vec::new();
        let mut requirements_satisfied: Vec<String> = Vec::new();

        for line in output.lines() {
            let line = line.trim();

            // Skip progress lines
            if line.starts_with("Collecting")
                || line.starts_with("Downloading")
                || line.starts_with("Using cached")
                || line.contains("━━━━━━━━━━")
                || line.contains("████████")
                || line.starts_with("Preparing metadata")
                || line.starts_with("Building wheel")
                || line.starts_with("Getting requirements")
                || line.starts_with("Preparing metadata")
            {
                continue;
            }

            // Track installed packages
            if let Some(rest) = line.strip_prefix("Successfully installed ") {
                // Parse: Successfully installed package1-1.0 package2-2.0
                let packages: Vec<&str> = rest.split_whitespace().collect();
                for pkg in packages {
                    installed.push(pkg.to_string());
                }
            }

            // Track requirements already satisfied
            if line.starts_with("Requirement already satisfied:") {
                // Parse: Requirement already satisfied: package in /path/to/lib
                if let Some(pkg_part) = line.split(':').nth(1) {
                    let pkg_name = pkg_part.split_whitespace().next().unwrap_or("");
                    if !pkg_name.is_empty() {
                        requirements_satisfied.push(pkg_name.to_string());
                    }
                }
            }
        }

        if !installed.is_empty() {
            result.push(format!("Installed: {}", installed.join(", ")));
        }

        if !requirements_satisfied.is_empty() {
            // Dedupe and limit
            requirements_satisfied.sort();
            requirements_satisfied.dedup();
            if requirements_satisfied.len() > 10 {
                result.push(format!(
                    "Already satisfied: {} packages",
                    requirements_satisfied.len()
                ));
            } else {
                result.push(format!(
                    "Already satisfied: {}",
                    requirements_satisfied.join(", ")
                ));
            }
        }

        if result.is_empty() {
            return Ok("(installed)".to_string());
        }

        Ok(result.join("\n"))
    }

    /// Handle pip uninstall - silent on success
    fn handle_uninstall(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 {
            // Extract what was uninstalled
            let uninstalled: Vec<&str> = output
                .lines()
                .filter(|line| line.starts_with("Successfully uninstalled"))
                .collect();

            if !uninstalled.is_empty() {
                return Ok(uninstalled.join("\n"));
            }

            return Ok(String::new());
        }

        self.error_strategy.compress(output)
    }

    /// Handle pip show - format package info
    fn handle_show(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(package not found)".to_string());
        }

        // Parse key-value format
        let mut info: Vec<(&str, &str)> = Vec::new();

        for line in output.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                if !value.is_empty() {
                    info.push((key, value));
                }
            }
        }

        if info.is_empty() {
            return Ok("(package not found)".to_string());
        }

        // Format output with key fields
        let mut result = Vec::new();
        let important_keys = ["Name", "Version", "Summary", "Location"];

        for (key, value) in &info {
            if important_keys.contains(key) {
                result.push(format!("{}: {}", key, value));
            }
        }

        // Add other fields if not too many
        let other_info: Vec<_> = info
            .iter()
            .filter(|(k, _)| !important_keys.contains(k))
            .take(5)
            .collect();

        for (key, value) in other_info {
            result.push(format!("{}: {}", key, value));
        }

        Ok(result.join("\n"))
    }

    /// Handle pip freeze - format requirements
    fn handle_freeze(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no packages)".to_string());
        }

        let packages: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();

        if packages.is_empty() {
            return Ok("(no packages)".to_string());
        }

        let count = packages.len();
        let mut result = vec![format!("{} packages frozen", count)];

        for pkg in packages.iter().take(20) {
            result.push(format!("  {}", pkg));
        }

        if count > 20 {
            result.push(format!("  ... and {} more", count - 20));
        }

        Ok(result.join("\n"))
    }
}

impl Default for PipModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for PipModule {
    fn name(&self) -> &str {
        "pip"
    }

    fn strategy(&self) -> &str {
        "multi_strategy"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        let subcommand = context
            .command
            .as_ref()
            .and_then(|cmd| self.detect_subcommand(cmd));

        // Check for --outdated flag in command
        let is_outdated = context
            .command
            .as_ref()
            .map(|cmd| cmd.contains("--outdated"))
            .unwrap_or(false);

        match subcommand.as_deref() {
            Some("list") if is_outdated => self.handle_outdated(output),
            Some("list") => self.handle_list(output),
            Some("install") => self.handle_install(output, context.exit_code),
            Some("uninstall") => self.handle_uninstall(output, context.exit_code),
            Some("show") => self.handle_show(output),
            Some("freeze") => self.handle_freeze(output),
            _ => {
                // Default to grouping for unknown pip commands
                self.grouping_strategy.compress(output)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(command: &str, exit_code: i32) -> Context {
        Context {
            cwd: "/tmp".to_string(),
            exit_code,
            tool: "bash".to_string(),
            session_id: None,
            command: Some(command.to_string()),
        }
    }

    #[test]
    fn test_pip_list() {
        let module = PipModule::new();
        let input = r#"Package          Version
---------------- -------
requests         2.31.0
urllib3          2.0.4
certifi          2023.7.22
"#;
        let result = module
            .compress(input, &make_context("pip list", 0))
            .unwrap();

        assert!(result.contains("3 packages"));
        assert!(result.contains("requests==2.31.0"));
        assert!(result.contains("urllib3==2.0.4"));
    }

    #[test]
    fn test_pip_list_empty() {
        let module = PipModule::new();
        let result = module.compress("", &make_context("pip list", 0)).unwrap();

        assert_eq!(result, "(no packages)");
    }

    #[test]
    fn test_pip_list_outdated() {
        let module = PipModule::new();
        let input = r#"Package      Version Latest Type
------------ ------- ------ -----
requests     2.28.0  2.31.0 wheel
urllib3      1.26.0  2.0.4  wheel
"#;
        let result = module
            .compress(input, &make_context("pip list --outdated", 0))
            .unwrap();

        assert!(result.contains("2 outdated"));
        assert!(result.contains("requests 2.28.0 -> 2.31.0"));
        assert!(result.contains("urllib3 1.26.0 -> 2.0.4"));
    }

    #[test]
    fn test_pip_list_outdated_empty() {
        let module = PipModule::new();
        let input = "";
        let result = module
            .compress(input, &make_context("pip list --outdated", 0))
            .unwrap();

        assert_eq!(result, "(all packages up to date)");
    }

    #[test]
    fn test_pip_install_success() {
        let module = PipModule::new();
        let input = r#"Collecting requests
  Using cached requests-2.31.0-py3-none-any.whl (62 kB)
Installing collected packages: requests
Successfully installed requests-2.31.0
"#;
        let result = module
            .compress(input, &make_context("pip install requests", 0))
            .unwrap();

        assert!(result.contains("Installed: requests-2.31.0"));
        assert!(!result.contains("Collecting"));
        assert!(!result.contains("Using cached"));
    }

    #[test]
    fn test_pip_install_already_satisfied() {
        let module = PipModule::new();
        let input = r#"Requirement already satisfied: requests in /usr/lib/python3.11/site-packages (2.31.0)
Requirement already satisfied: charset-normalizer in /usr/lib/python3.11/site-packages (from requests)
"#;
        let result = module
            .compress(input, &make_context("pip install requests", 0))
            .unwrap();

        assert!(result.contains("Already satisfied"));
        assert!(result.contains("requests"));
    }

    #[test]
    fn test_pip_install_error() {
        let module = PipModule::new();
        let input = r#"ERROR: Could not find a version that satisfies the requirement nonexistent-package
ERROR: No matching distribution found for nonexistent-package
"#;
        let result = module
            .compress(input, &make_context("pip install nonexistent-package", 1))
            .unwrap();

        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_pip_uninstall_success() {
        let module = PipModule::new();
        let input = "Successfully uninstalled requests-2.31.0";
        let result = module
            .compress(input, &make_context("pip uninstall requests", 0))
            .unwrap();

        assert!(result.contains("Successfully uninstalled"));
    }

    #[test]
    fn test_pip_uninstall_silent() {
        let module = PipModule::new();
        let result = module
            .compress("", &make_context("pip uninstall requests", 0))
            .unwrap();

        assert_eq!(result, "");
    }

    #[test]
    fn test_pip_show() {
        let module = PipModule::new();
        let input = r#"Name: requests
Version: 2.31.0
Summary: Python HTTP for Humans.
Author: Kenneth Reitz
Author-email: me@kennethreitz.org
License: Apache 2.0
Location: /usr/lib/python3.11/site-packages
Requires: certifi, charset-normalizer, idna, urllib3
"#;
        let result = module
            .compress(input, &make_context("pip show requests", 0))
            .unwrap();

        assert!(result.contains("Name: requests"));
        assert!(result.contains("Version: 2.31.0"));
        assert!(result.contains("Summary:"));
    }

    #[test]
    fn test_pip_show_not_found() {
        let module = PipModule::new();
        let result = module
            .compress("", &make_context("pip show nonexistent", 0))
            .unwrap();

        assert_eq!(result, "(package not found)");
    }

    #[test]
    fn test_pip_freeze() {
        let module = PipModule::new();
        let input = r#"requests==2.31.0
urllib3==2.0.4
certifi==2023.7.22
"#;
        let result = module
            .compress(input, &make_context("pip freeze", 0))
            .unwrap();

        assert!(result.contains("3 packages frozen"));
        assert!(result.contains("requests==2.31.0"));
    }

    #[test]
    fn test_pip_freeze_empty() {
        let module = PipModule::new();
        let result = module.compress("", &make_context("pip freeze", 0)).unwrap();

        assert_eq!(result, "(no packages)");
    }

    #[test]
    fn test_pip_unknown_command() {
        let module = PipModule::new();
        let input = "some output";
        let result = module
            .compress(input, &make_context("pip unknown", 0))
            .unwrap();

        // Should fall back to grouping
        assert!(!result.is_empty());
    }

    #[test]
    fn test_pip_list_many_packages() {
        let module = PipModule::new();
        let mut input = String::from("Package          Version\n---------------- -------\n");
        for i in 1..=30 {
            input.push_str(&format!("package{}       1.0.{}\n", i, i));
        }

        let result = module
            .compress(&input, &make_context("pip list", 0))
            .unwrap();

        assert!(result.contains("30 packages"));
        assert!(result.contains("and 10 more")); // 30 - 20 = 10
    }
}
