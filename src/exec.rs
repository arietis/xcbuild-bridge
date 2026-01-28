use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub terminated_by_signal: bool,
    pub stdout: String,
    pub stderr: String,
}

pub trait Runner {
    fn run(&self, spec: &CommandSpec) -> io::Result<CommandOutput>;
}

pub struct OsRunner;

impl Runner for OsRunner {
    fn run(&self, spec: &CommandSpec) -> io::Result<CommandOutput> {
        let mut cmd = std::process::Command::new(&spec.program);
        cmd.args(&spec.args);
        if let Some(cwd) = &spec.cwd {
            cmd.current_dir(cwd);
        }
        if let Some(env) = &spec.env {
            cmd.envs(env);
        }
        let output = cmd.output()?;
        let terminated_by_signal = output.status.code().is_none();
        let exit_code = output.status.code().unwrap_or(-1);
        Ok(CommandOutput {
            exit_code,
            terminated_by_signal,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
