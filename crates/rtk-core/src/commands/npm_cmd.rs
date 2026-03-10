use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;

pub struct NpmModule {
    strategy: ErrorOnly,
}

impl NpmModule {
    pub fn new() -> Self {
        Self {
            strategy: ErrorOnly,
        }
    }
}

impl Default for NpmModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for NpmModule {
    fn name(&self) -> &str {
        "npm"
    }

    fn strategy(&self) -> &str {
        self.strategy.name()
    }

    fn compress(&self, output: &str, _context: &Context) -> Result<String> {
        self.strategy.compress(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npm_test_failures() {
        let module = NpmModule::new();
        let input = r#"Test 1 passed
Test 2 passed
Error: Test 3 failed
Test 4 passed
"#;
        let result = module
            .compress(
                input,
                &Context {
                    cwd: "/tmp".to_string(),
                    exit_code: 0,
                    tool: "bash".to_string(),
                    session_id: None,
                    command: Some("npm test".to_string()),
                },
            )
            .unwrap();

        assert!(result.contains("Error: Test 3 failed"));
        assert!(!result.contains("Test 1 passed"));
    }
}
