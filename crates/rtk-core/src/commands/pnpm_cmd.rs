use crate::{
    commands::CommandModule,
    filter::{GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct PnpmModule {
    strategy: GroupingByPattern,
}

impl PnpmModule {
    pub fn new() -> Self {
        Self {
            strategy: GroupingByPattern,
        }
    }
}

impl Default for PnpmModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for PnpmModule {
    fn name(&self) -> &str {
        "pnpm"
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
        let module = PnpmModule::new();
        let result = module.compress("", &make_context("pnpm")).unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_pnpm_install_success() {
        let module = PnpmModule::new();
        let input = r#"Lockfile is up to date, resolution step is skipped
Already up to date

packages: audited 150 packages in 2.5s

150 packages are looking for funding
  run `npm fund` for details
"#;
        let result = module
            .compress(input, &make_context("pnpm install"))
            .unwrap();

        // Should use grouping strategy
        assert!(!result.is_empty());
    }

    #[test]
    fn test_pnpm_install_progress() {
        let module = PnpmModule::new();
        let input = r#"Progress: resolved 1, reused 1, downloaded 0, added 0, done
Progress: resolved 2, reused 2, downloaded 0, added 0, done
Progress: resolved 3, reused 3, downloaded 0, added 0, done
Progress: resolved 4, reused 4, downloaded 0, added 0, done
Progress: resolved 5, reused 5, downloaded 0, added 0, done

packages: audited 5 packages in 1.2s
"#;
        let result = module
            .compress(input, &make_context("pnpm install"))
            .unwrap();

        // Should group repetitive progress lines
        assert!(!result.is_empty());
    }

    #[test]
    fn test_pnpm_add_package() {
        let module = PnpmModule::new();
        let input = r#" WARN   2 deprecated subdependencies found: glob@7.2.3, inflight@1.0.6
Packages: +1
+
Progress: resolved 1, reused 1, downloaded 0, added 1, done

dependencies:
+ lodash 4.17.21

Done in 1.8s
"#;
        let result = module
            .compress(input, &make_context("pnpm add lodash"))
            .unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_pnpm_update() {
        let module = PnpmModule::new();
        let input = r#"Progress: resolved 1, reused 0, downloaded 1, added 1, done
Progress: resolved 2, reused 1, downloaded 1, added 1, done
Progress: resolved 3, reused 2, downloaded 1, added 1, done

dependencies:
- typescript 5.0.0
+ typescript 5.2.0

Done in 2.1s
"#;
        let result = module
            .compress(input, &make_context("pnpm update typescript"))
            .unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_pnpm_run_script() {
        let module = PnpmModule::new();
        let input = r#"> myproject@1.0.0 build
> next build

  ▲ Next.js 14.0.0
   - Local:        http://localhost:3000

   Creating an optimized production build ...
   Compiled successfully
   ✓ Compiled successfully
"#;
        let result = module
            .compress(input, &make_context("pnpm run build"))
            .unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_pnpm_list() {
        let module = PnpmModule::new();
        let input = r#"Legend: production dependency, optional only, dev only

myproject@1.0.0 /home/user/project

dependencies:
next 14.0.0
react 18.2.0
react-dom 18.2.0

devDependencies:
typescript 5.2.0
eslint 8.50.0
"#;
        let result = module.compress(input, &make_context("pnpm list")).unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_pnpm_outdated() {
        let module = PnpmModule::new();
        let input = r#"Package       Current  Latest  Dependency
next          14.0.0   14.1.0  dependencies
react         18.2.0   18.3.0  dependencies
typescript    5.2.0    5.3.0   devDependencies
"#;
        let result = module
            .compress(input, &make_context("pnpm outdated"))
            .unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_pnpm_download_progress() {
        let module = PnpmModule::new();
        // Simulate repetitive download progress
        let input = r#"Downloading next@14.0.0: 1.23 MB/5.00 MB
Downloading next@14.0.0: 2.45 MB/5.00 MB
Downloading next@14.0.0: 3.67 MB/5.00 MB
Downloading next@14.0.0: 4.89 MB/5.00 MB
Downloading next@14.0.0: 5.00 MB/5.00 MB

packages: +1
+ next 14.0.0
"#;
        let result = module
            .compress(input, &make_context("pnpm install"))
            .unwrap();

        // Should group similar download lines
        assert!(!result.is_empty());
    }

    #[test]
    fn test_pnpm_build_progress() {
        let module = PnpmModule::new();
        let input = r#"Building dependency graph...
Building dependency graph...
Building dependency graph...

Resolving packages...
Resolving packages...

Fetching packages...
Fetching packages...

Done in 5.2s
"#;
        let result = module
            .compress(input, &make_context("pnpm install"))
            .unwrap();

        // Should group repetitive status messages
        assert!(!result.is_empty());
    }
}
