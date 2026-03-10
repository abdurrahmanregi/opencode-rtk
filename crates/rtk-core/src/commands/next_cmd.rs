use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;

pub struct NextModule {
    strategy: ErrorOnly,
}

impl NextModule {
    pub fn new() -> Self {
        Self {
            strategy: ErrorOnly,
        }
    }
}

impl Default for NextModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for NextModule {
    fn name(&self) -> &str {
        "next"
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
        let module = NextModule::new();
        let result = module.compress("", &make_context("next build")).unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_next_build_success() {
        let module = NextModule::new();
        let input = r#"  ▲ Next.js 14.0.0
   - Environments: .env.local
   - Experiments (use with caution): 
     · serverActions

   Creating an optimized production build ...
   Compiled successfully

   Linting and checking validity of types ...
   Compiled successfully

   Collecting page data ...
   Generating static pages (0/5)
   Generating static pages (1/5)
   Generating static pages (2/5)
   Generating static pages (3/5)
   Generating static pages (4/5)
   Finalizing page optimization ...

   Route (app)                              Size     First Load JS
   ┌ ○ /                                    5.2 kB         89 kB
   ├ ○ /about                               2.1 kB         86 kB
   └ ○ /contact                             1.8 kB         85 kB
   + First Load JS shared by all            83.9 kB

○  (Static)  prerendered as static content

✓ Compiled successfully
✓ Linting and checking validity of types
✓ Collecting page data
✓ Generating static pages (1/1)
✓ Finalizing page optimization
"#;
        let result = module.compress(input, &make_context("next build")).unwrap();

        // No errors, should return "(no errors)"
        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_next_build_error() {
        let module = NextModule::new();
        let input = r#"  ▲ Next.js 14.0.0

   Creating an optimized production build ...
   Failed to compile.

   ./src/app/page.tsx
   Error: x is not defined
   
   CompileError: Failed to compile
"#;
        let result = module.compress(input, &make_context("next build")).unwrap();

        // Should contain error lines
        assert!(result.contains("Error") || result.contains("Failed"));
    }

    #[test]
    fn test_next_dev_output() {
        let module = NextModule::new();
        let input = r#"  ▲ Next.js 14.0.0
   - Local:        http://localhost:3000
   - Environments: .env.local

   ✓ Ready in 2.5s
   ✓ Compiled / in 1.2s (500 modules)
   ○ Compiling /about ...
   ✓ Compiled /about in 800ms (350 modules)
"#;
        let result = module.compress(input, &make_context("next dev")).unwrap();

        // No errors
        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_next_dev_error() {
        let module = NextModule::new();
        let input = r#"  ▲ Next.js 14.0.0
   - Local:        http://localhost:3000

   ✓ Ready in 2.5s
   ○ Compiling /error ...
   Error: Cannot find module './missing-file'
   Failed to compile
"#;
        let result = module.compress(input, &make_context("next dev")).unwrap();

        assert!(result.contains("Error") || result.contains("Failed"));
    }

    #[test]
    fn test_next_export_error() {
        let module = NextModule::new();
        let input = r#"   Collecting page data ...
   Error: Export encountered errors on following paths:
        /error-page
   Build error occurred
"#;
        let result = module
            .compress(input, &make_context("next export"))
            .unwrap();

        assert!(result.contains("Error"));
    }
}
