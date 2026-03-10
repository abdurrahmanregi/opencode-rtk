use anyhow::{Context, Result};
use std::process::{Command, Output};

pub fn execute_command(program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute: {} {:?}", program, args))
}

pub fn execute_command_in_dir(program: &str, args: &[&str], cwd: &str) -> Result<Output> {
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to execute: {} {:?} in {}", program, args, cwd))
}
