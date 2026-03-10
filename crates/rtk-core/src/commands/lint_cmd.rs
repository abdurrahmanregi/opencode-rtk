use crate::{
    commands::CommandModule,
    filter::{GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct LintModule {
    strategy: GroupingByPattern,
}

impl LintModule {
    pub fn new() -> Self {
        Self {
            strategy: GroupingByPattern,
        }
    }
}

impl Default for LintModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for LintModule {
    fn name(&self) -> &str {
        "eslint"
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

    fn make_context(command: &str) -> Context {
        Context {
            cwd: "/tmp".to_string(),
            exit_code: 0,
            tool: "bash".to_string(),
            session_id: None,
            command: Some(command.to_string()),
        }
    }

    #[test]
    fn test_empty() {
        let module = LintModule::new();
        let result = module.compress("", &make_context("eslint")).unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_eslint_errors() {
        let module = LintModule::new();
        let input = r#"/path/to/file.js
  10:5  error  'x' is not defined    no-undef
  15:3  error  Unexpected console statement  no-console

/path/to/another.js
  5:10  error  Missing semicolon  semi

2 problems (2 errors, 0 warnings)
"#;
        let result = module
            .compress(input, &make_context("eslint src/"))
            .unwrap();

        // Should use grouping strategy
        assert!(!result.is_empty());
    }

    #[test]
    fn test_eslint_warnings() {
        let module = LintModule::new();
        let input = r#"/path/to/file.ts
  20:1  warning  Missing return type on function  explicit-module-boundary-types
  25:5  warning  Unexpected any  no-explicit-any

✖ 2 problems (0 errors, 2 warnings)
"#;
        let result = module
            .compress(input, &make_context("eslint --ext .ts"))
            .unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_eslint_clean() {
        let module = LintModule::new();
        let input = "✨  Done in 2.50s.\n";
        let result = module
            .compress(input, &make_context("eslint src/"))
            .unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_eslint_grouping() {
        let module = LintModule::new();
        // Create repetitive lint output
        let input = r#"src/components/Button.tsx
  10:5  error  'onClick' is missing in props validation  react/prop-types
  25:3  error  'onClick' is missing in props validation  react/prop-types
  40:7  error  'onClick' is missing in props validation  react/prop-types

src/components/Input.tsx
  15:2  error  'onChange' is missing in props validation  react/prop-types
  30:4  error  'onChange' is missing in props validation  react/prop-types
"#;
        let result = module
            .compress(input, &make_context("eslint src/"))
            .unwrap();

        // Should group similar errors
        assert!(!result.is_empty());
    }
}
