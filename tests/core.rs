use std::cell::RefCell;
use std::io;

use xcbuild_bridge::{
    build_command, execute, validate_request, CommandSpec, RunRequest, RunResult, Runner,
    ValidationError,
};

struct FakeRunner {
    last_spec: RefCell<Option<CommandSpec>>,
    result: Result<RunResult, io::ErrorKind>,
}

impl FakeRunner {
    fn ok(run: RunResult) -> Self {
        FakeRunner {
            last_spec: RefCell::new(None),
            result: Ok(run),
        }
    }

    fn err(kind: io::ErrorKind) -> Self {
        FakeRunner {
            last_spec: RefCell::new(None),
            result: Err(kind),
        }
    }

    fn last_spec(&self) -> Option<CommandSpec> {
        self.last_spec.borrow().clone()
    }
}

impl Runner for FakeRunner {
    fn run(&self, spec: &CommandSpec) -> io::Result<RunResult> {
        *self.last_spec.borrow_mut() = Some(spec.clone());
        match &self.result {
            Ok(run) => Ok(run.clone()),
            Err(kind) => Err(io::Error::new(*kind, "fake")),
        }
    }
}

#[test]
fn validate_rejects_empty_args() {
    let req = RunRequest {
        args: vec![],
        cwd: None,
    };
    assert_eq!(validate_request(req), Err(ValidationError::EmptyArgs));
}

#[test]
fn validate_rejects_nul_bytes() {
    let req = RunRequest {
        args: vec!["a\0b".into()],
        cwd: None,
    };
    assert_eq!(validate_request(req), Err(ValidationError::NulByte));
}

#[test]
fn build_command_sets_program_and_args() {
    let req = RunRequest {
        args: vec!["arg1".into()],
        cwd: None,
    };
    let valid = validate_request(req).unwrap();
    let spec = build_command(valid);
    assert_eq!(spec.program, "xcodebuild");
    assert_eq!(spec.args, vec!["arg1".to_string()]);
}

#[test]
fn execute_success_returns_ok_and_exit_code() {
    let runner = FakeRunner::ok(RunResult {
        exit_code: 0,
        terminated_by_signal: false,
        stdout: "out".into(),
        stderr: "".into(),
    });
    let req = RunRequest {
        args: vec!["arg1".into()],
        cwd: None,
    };
    let resp = execute(req, &runner);
    assert!(resp.ok);
    assert_eq!(resp.exit_code, 0);
    let spec = runner.last_spec().unwrap();
    assert_eq!(spec.program, "xcodebuild");
    assert_eq!(spec.args, vec!["arg1".to_string()]);
}

#[test]
fn execute_validation_error_returns_error() {
    let runner = FakeRunner::ok(RunResult {
        exit_code: 0,
        terminated_by_signal: false,
        stdout: "".into(),
        stderr: "".into(),
    });
    let req = RunRequest {
        args: vec![],
        cwd: None,
    };
    let resp = execute(req, &runner);
    assert!(!resp.ok);
    assert!(resp.error.unwrap().contains("validation_failed"));
}

#[test]
fn execute_runner_error_returns_error() {
    let runner = FakeRunner::err(io::ErrorKind::Other);
    let req = RunRequest {
        args: vec!["arg1".into()],
        cwd: None,
    };
    let resp = execute(req, &runner);
    assert!(!resp.ok);
    assert!(resp.error.unwrap().contains("spawn_failed"));
}
