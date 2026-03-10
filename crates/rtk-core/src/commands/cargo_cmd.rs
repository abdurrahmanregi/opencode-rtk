use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;

pub struct CargoModule {
    strategy: ErrorOnly,
}

impl CargoModule {
    pub fn new() -> Self {
        Self {
            strategy: ErrorOnly,
        }
    }
}

impl Default for CargoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for CargoModule {
    fn name(&self) -> &str {
        "cargo"
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
    fn test_cargo_build_errors() {
        let module = CargoModule::new();
        let input = r#"Compiling v0.1.0
error: could not compile
warning: unused variable
"#;
        let result = module
            .compress(
                input,
                &Context {
                    cwd: "/tmp".to_string(),
                    exit_code: 0,
                    tool: "bash".to_string(),
                    session_id: None,
                    command: Some("cargo build".to_string()),
                },
            )
            .unwrap();

        assert!(result.contains("error: could not compile"));
    }
}
