use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;

pub struct VitestModule {
    strategy: ErrorOnly,
}

impl VitestModule {
    pub fn new() -> Self {
        Self {
            strategy: ErrorOnly,
        }
    }
}

impl Default for VitestModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for VitestModule {
    fn name(&self) -> &str {
        "vitest"
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
        let module = VitestModule::new();
        let result = module.compress("", &make_context("vitest")).unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_vitest_all_passed() {
        let module = VitestModule::new();
        let input = r#" RUN  v1.2.0 /home/user/project

 ✓ src/__tests__/sum.test.ts (5)
 ✓ src/__tests__/utils.test.ts (10)
 ✓ src/__tests__/api.test.ts (8)

 Test Files  3 passed (3)
      Tests  23 passed (23)
   Start at  12:34:56
   Duration  1.23s (transform 0.5s, setup 0.2s, collect 0.3s, tests 0.8s)
"#;
        let result = module.compress(input, &make_context("vitest run")).unwrap();

        // No errors
        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_vitest_failures() {
        let module = VitestModule::new();
        let input = r#" RUN  v1.2.0 /home/user/project

 ✓ src/__tests__/sum.test.ts (5)
 ✗ src/__tests__/utils.test.ts (3)
 ✓ src/__tests__/api.test.ts (8)

 FAIL  src/__tests__/utils.test.ts > formatDate > should format date correctly
AssertionError: expected '2024-01-01' to be '2024-01-02'

Expected: "2024-01-02"
Received: "2024-01-01"

 ❯ src/__tests__/utils.test.ts:15:5

 FAIL  src/__tests__/utils.test.ts > parseJSON > should parse valid JSON
SyntaxError: Unexpected token in JSON

 ❯ src/__tests__/utils.test.ts:25:10

 Test Files  1 failed | 2 passed (3)
      Tests  2 failed | 21 passed (23)
   Duration  1.45s
"#;
        let result = module.compress(input, &make_context("vitest run")).unwrap();

        // Should contain failure and error information
        assert!(result.contains("failed") || result.contains("Failed") || result.contains("Error"));
    }

    #[test]
    fn test_vitest_timeout() {
        let module = VitestModule::new();
        let input = r#" RUN  v1.2.0 /home/user/project

 ✓ src/__tests__/fast.test.ts (5)
 ✗ src/__tests__/slow.test.ts (1)

 FAIL  src/__tests__/slow.test.ts > slowOperation > should complete within timeout
Error: Test timed out in 5000ms.

 ❯ src/__tests__/slow.test.ts:10:5

 Test Files  1 failed | 1 passed (2)
      Tests  1 failed | 5 passed (6)
"#;
        let result = module.compress(input, &make_context("vitest run")).unwrap();

        assert!(result.contains("Error") || result.contains("failed"));
    }

    #[test]
    fn test_vitest_watch_mode() {
        let module = VitestModule::new();
        let input = r#" RUN  v1.2.0 /home/user/project

 ✓ src/__tests__/sum.test.ts (5)

 Test Files  1 passed (1)
      Tests  5 passed (5)
   Duration  0.85s

 WATCH  Waiting for file changes...
"#;
        let result = module.compress(input, &make_context("vitest")).unwrap();

        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_vitest_coverage() {
        let module = VitestModule::new();
        let input = r#" RUN  v1.2.0 /home/user/project with coverage

 ✓ src/__tests__/sum.test.ts (5)
 ✓ src/__tests__/utils.test.ts (10)

 Test Files  2 passed (2)
      Tests  15 passed (15)
   Coverage   85.23%
   Duration  2.34s
"#;
        let result = module
            .compress(input, &make_context("vitest run --coverage"))
            .unwrap();

        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_vitest_snapshot_failure() {
        let module = VitestModule::new();
        let input = r#" RUN  v1.2.0 /home/user/project

 ✗ src/__tests__/component.test.ts (2)

 FAIL  src/__tests__/component.test.ts > render > should match snapshot
AssertionError: Snapshot `render should match snapshot 1` outdated

- Snapshot  - 5
+ Received  + 5

  <div class="old-class">
    Old content
  </div>

  <div class="new-class">
    New content
  </div>

 ❯ src/__tests__/component.test.ts:20:5

 Test Files  1 failed (1)
      Tests  1 failed (1)
"#;
        let result = module.compress(input, &make_context("vitest run")).unwrap();

        assert!(result.contains("failed") || result.contains("Error"));
    }

    #[test]
    fn test_vitest_parallel_execution() {
        let module = VitestModule::new();
        let input = r#" RUN  v1.2.0 /home/user/project

 ✓ src/__tests__/test1.test.ts (10) 234ms
 ✓ src/__tests__/test2.test.ts (8) 198ms
 ✓ src/__tests__/test3.test.ts (12) 312ms
 ✓ src/__tests__/test4.test.ts (6) 156ms

 Test Files  4 passed (4)
      Tests  36 passed (36)
   Duration  1.12s
"#;
        let result = module.compress(input, &make_context("vitest run")).unwrap();

        assert_eq!(result, "(no errors)");
    }
}
