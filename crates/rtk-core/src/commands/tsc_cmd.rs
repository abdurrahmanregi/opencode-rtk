use crate::{
    commands::CommandModule,
    filter::{GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct TscModule {
    strategy: GroupingByPattern,
}

impl TscModule {
    pub fn new() -> Self {
        Self {
            strategy: GroupingByPattern,
        }
    }
}

impl Default for TscModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for TscModule {
    fn name(&self) -> &str {
        "tsc"
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
        let module = TscModule::new();
        let result = module.compress("", &make_context("tsc")).unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_tsc_errors() {
        let module = TscModule::new();
        let input = r#"src/index.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.
src/utils.ts(25,10): error TS2339: Property 'foo' does not exist on type 'Bar'.
src/components/App.tsx(50,3): error TS2741: Property 'name' is missing in type '{}' but required in type 'Props'.

Found 3 errors.
"#;
        let result = module
            .compress(input, &make_context("tsc --noEmit"))
            .unwrap();

        // Should use grouping strategy
        assert!(!result.is_empty());
    }

    #[test]
    fn test_tsc_type_errors() {
        let module = TscModule::new();
        let input = r#"src/api.ts(15,7): error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'.
src/api.ts(20,12): error TS2345: Argument of type 'undefined' is not assignable to parameter of type 'string'.
src/api.ts(35,5): error TS2345: Argument of type 'null' is not assignable to parameter of type 'object'.
"#;
        let result = module.compress(input, &make_context("tsc")).unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_tsc_clean() {
        let module = TscModule::new();
        let input = "";
        let result = module
            .compress(input, &make_context("tsc --noEmit"))
            .unwrap();

        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_tsc_grouping() {
        let module = TscModule::new();
        // Create repetitive TypeScript errors
        let input = r#"src/module1.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.
src/module2.ts(15,3): error TS2322: Type 'string' is not assignable to type 'number'.
src/module3.ts(20,7): error TS2322: Type 'string' is not assignable to type 'number'.
src/module4.ts(25,9): error TS2322: Type 'string' is not assignable to type 'number'.
"#;
        let result = module.compress(input, &make_context("tsc")).unwrap();

        // Should group similar errors
        assert!(!result.is_empty());
    }

    #[test]
    fn test_tsc_with_context() {
        let module = TscModule::new();
        let input = r#"src/app.ts(100,5): error TS2304: Cannot find name 'React'.
src/app.ts(105,10): error TS2304: Cannot find name 'Component'.
src/app.ts(110,15): error TS2304: Cannot find name 'useState'.
"#;
        let result = module
            .compress(input, &make_context("tsc --noEmit"))
            .unwrap();

        assert!(!result.is_empty());
    }
}
