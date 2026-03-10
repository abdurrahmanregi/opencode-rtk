use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;

pub struct PlaywrightModule {
    strategy: ErrorOnly,
}

impl PlaywrightModule {
    pub fn new() -> Self {
        Self {
            strategy: ErrorOnly,
        }
    }
}

impl Default for PlaywrightModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for PlaywrightModule {
    fn name(&self) -> &str {
        "playwright"
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
        let module = PlaywrightModule::new();
        let result = module
            .compress("", &make_context("playwright test"))
            .unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_playwright_all_passed() {
        let module = PlaywrightModule::new();
        let input = r#"Running 5 tests using 1 worker

  ✓  1 [chromium] › login.spec.ts:10:5 › Login page (2s)
  ✓  2 [chromium] › dashboard.spec.ts:15:3 › Dashboard loads (1.5s)
  ✓  3 [chromium] › profile.spec.ts:20:7 › Profile update (2.2s)
  ✓  4 [chromium] › settings.spec.ts:12:5 › Settings save (1.8s)
  ✓  5 [chromium] › logout.spec.ts:8:3 › Logout works (1s)

  5 passed (8.5s)
"#;
        let result = module
            .compress(input, &make_context("playwright test"))
            .unwrap();

        // No errors
        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_playwright_failures() {
        let module = PlaywrightModule::new();
        let input = r#"Running 5 tests using 1 worker

  ✓  1 [chromium] › login.spec.ts:10:5 › Login page (2s)
  ✘  2 [chromium] › dashboard.spec.ts:15:3 › Dashboard loads (1.5s)
  ✓  3 [chromium] › profile.spec.ts:20:7 › Profile update (2.2s)
  ✘  4 [chromium] › settings.spec.ts:12:5 › Settings save (1.8s)
  ✓  5 [chromium] › logout.spec.ts:8:3 › Logout works (1s)

  2 failed
  3 passed
  5 total

  Failed tests:
    [chromium] › dashboard.spec.ts:15:3 › Dashboard loads
    Error: expect(received).toBeVisible()
    
    [chromium] › settings.spec.ts:12:5 › Settings save
    Error: Timed out waiting for element
"#;
        let result = module
            .compress(input, &make_context("playwright test"))
            .unwrap();

        // Should contain failure and error information
        assert!(result.contains("failed") || result.contains("Failed") || result.contains("Error"));
    }

    #[test]
    fn test_playwright_timeout_error() {
        let module = PlaywrightModule::new();
        let input = r#"Running 3 tests using 1 worker

  ✓  1 [chromium] › test1.spec.ts:5:3 › Test 1 (1s)
  ✘  2 [chromium] › test2.spec.ts:10:5 › Test 2 (30s)

  1) [chromium] › test2.spec.ts:10:5 › Test 2

    Error: page.goto: Timeout 30000ms exceeded.
    waiting for navigation to "https://example.com/slow" until "load"

  1 failed
  1 passed
"#;
        let result = module
            .compress(input, &make_context("playwright test"))
            .unwrap();

        assert!(result.contains("Error") || result.contains("failed"));
    }

    #[test]
    fn test_playwright_assertion_error() {
        let module = PlaywrightModule::new();
        let input = r#"Running 2 tests using 1 worker

  ✘  1 [chromium] › example.spec.ts:15:5 › Example test

    Error: expect(received).toBe(expected)

    Expected: "Welcome"
    Received: "Error"

    at example.spec.ts:17:10

  1 failed
"#;
        let result = module
            .compress(input, &make_context("playwright test"))
            .unwrap();

        assert!(result.contains("Error"));
    }

    #[test]
    fn test_playwright_multiple_browsers() {
        let module = PlaywrightModule::new();
        let input = r#"Running 6 tests using 3 workers

  ✓  1 [chromium] › test.spec.ts:5:3 › Test (1s)
  ✓  2 [firefox] › test.spec.ts:5:3 › Test (1.2s)
  ✘  3 [webkit] › test.spec.ts:5:3 › Test (1.5s)

  1 failed
  2 passed

  Failed:
    [webkit] › test.spec.ts:5:3 › Test
    Error: Element not found
"#;
        let result = module
            .compress(input, &make_context("playwright test"))
            .unwrap();

        assert!(result.contains("failed") || result.contains("Error"));
    }

    #[test]
    fn test_playwright_parallel_execution() {
        let module = PlaywrightModule::new();
        let input = r#"Running 10 tests using 4 workers

  ✓  1 [chromium] › spec1.ts:5:3 › Test 1 (1s)
  ✓  2 [chromium] › spec2.ts:5:3 › Test 2 (1.1s)
  ✓  3 [chromium] › spec3.ts:5:3 › Test 3 (0.9s)
  ✓  4 [chromium] › spec4.ts:5:3 › Test 4 (1.2s)
  ✓  5 [chromium] › spec5.ts:5:3 › Test 5 (1s)

  10 passed (3.2s)
"#;
        let result = module
            .compress(input, &make_context("playwright test"))
            .unwrap();

        assert_eq!(result, "(no errors)");
    }
}
