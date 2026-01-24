use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct RunRequest {
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedRequest {
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub exit_code: i32,
    pub terminated_by_signal: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResponse {
    pub ok: bool,
    pub exit_code: i32,
    pub terminated_by_signal: bool,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    EmptyArgs,
    NulByte,
}

pub fn validate_request(req: RunRequest) -> Result<ValidatedRequest, ValidationError> {
    if req.args.is_empty() {
        return Err(ValidationError::EmptyArgs);
    }
    if req.args.iter().any(|arg| arg.as_bytes().contains(&0)) {
        return Err(ValidationError::NulByte);
    }
    Ok(ValidatedRequest {
        args: req.args,
        cwd: req.cwd,
    })
}

pub fn build_command(valid: ValidatedRequest) -> CommandSpec {
    CommandSpec {
        program: "xcodebuild".to_string(),
        args: valid.args,
        cwd: valid.cwd,
    }
}

pub trait Runner {
    fn run(&self, spec: &CommandSpec) -> io::Result<RunResult>;
}

pub struct OsRunner;

impl Runner for OsRunner {
    fn run(&self, spec: &CommandSpec) -> io::Result<RunResult> {
        let mut cmd = std::process::Command::new(&spec.program);
        cmd.args(&spec.args);
        if let Some(cwd) = &spec.cwd {
            cmd.current_dir(cwd);
        }
        let output = cmd.output()?;
        let terminated_by_signal = output.status.code().is_none();
        let exit_code = output.status.code().unwrap_or(-1);
        Ok(RunResult {
            exit_code,
            terminated_by_signal,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

impl RunResponse {
    pub fn ok(run: RunResult) -> Self {
        RunResponse {
            ok: true,
            exit_code: run.exit_code,
            terminated_by_signal: run.terminated_by_signal,
            stdout: run.stdout,
            stderr: run.stderr,
            error: None,
        }
    }

    pub fn error(msg: String) -> Self {
        RunResponse {
            ok: false,
            exit_code: -1,
            terminated_by_signal: false,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(msg),
        }
    }
}

pub fn execute(req: RunRequest, runner: &impl Runner) -> RunResponse {
    match validate_request(req) {
        Ok(valid) => {
            let spec = build_command(valid);
            match runner.run(&spec) {
                Ok(run) => RunResponse::ok(run),
                Err(err) => RunResponse::error(format!("spawn_failed: {}", err)),
            }
        }
        Err(err) => RunResponse::error(format!("validation_failed: {:?}", err)),
    }
}
