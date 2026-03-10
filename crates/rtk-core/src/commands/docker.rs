use crate::{
    commands::CommandModule,
    filter::{GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct DockerModule {
    strategy: GroupingByPattern,
}

impl DockerModule {
    pub fn new() -> Self {
        Self {
            strategy: GroupingByPattern,
        }
    }
}

impl Default for DockerModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for DockerModule {
    fn name(&self) -> &str {
        "docker"
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
    fn test_docker_logs() {
        let module = DockerModule::new();
        let input = "Container starting...\nContainer starting...\nContainer starting...\n";
        let result = module
            .compress(
                input,
                &Context {
                    cwd: "/tmp".to_string(),
                    exit_code: 0,
                    tool: "bash".to_string(),
                    session_id: None,
                    command: Some("docker logs".to_string()),
                },
            )
            .unwrap();

        assert!(result.contains("occurrences"));
    }
}
