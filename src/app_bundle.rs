use std::fs;

use crate::exec::{CommandSpec, Runner};

pub fn extract_app_path_from_build_settings(output: &str) -> Option<String> {
    let built_products_dir = extract_setting(output, "BUILT_PRODUCTS_DIR")?;
    let full_product_name = extract_setting(output, "FULL_PRODUCT_NAME")?;
    Some(format!("{}/{}", built_products_dir, full_product_name))
}

pub fn validate_app_path(app_path: &str) -> Result<(), String> {
    let metadata = fs::metadata(app_path).map_err(|err| err.to_string())?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err("app path is not a directory".to_string())
    }
}

pub fn read_bundle_id(app_path: &str, runner: &impl Runner) -> Result<String, String> {
    let info_path = format!("{}/Info", app_path.trim_end_matches('/'));
    let defaults_spec = CommandSpec {
        program: "defaults".to_string(),
        args: vec![
            "read".to_string(),
            info_path,
            "CFBundleIdentifier".to_string(),
        ],
        cwd: None,
        env: None,
    };
    let defaults_output = runner.run(&defaults_spec).map_err(|err| err.to_string())?;
    if defaults_output.exit_code == 0 {
        let bundle_id = defaults_output.stdout.trim();
        if !bundle_id.is_empty() {
            return Ok(bundle_id.to_string());
        }
    }

    let plist_path = format!("{}/Info.plist", app_path.trim_end_matches('/'));
    let plist_spec = CommandSpec {
        program: "/usr/libexec/PlistBuddy".to_string(),
        args: vec![
            "-c".to_string(),
            "Print :CFBundleIdentifier".to_string(),
            plist_path,
        ],
        cwd: None,
        env: None,
    };
    let plist_output = runner.run(&plist_spec).map_err(|err| err.to_string())?;
    if plist_output.exit_code == 0 {
        let bundle_id = plist_output.stdout.trim();
        if !bundle_id.is_empty() {
            return Ok(bundle_id.to_string());
        }
    }

    Err(format!(
        "defaults output: {}\nplistbuddy output: {}",
        format_command_output(&defaults_output),
        format_command_output(&plist_output)
    ))
}

fn extract_setting(output: &str, key: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) {
            if let Some((_, value)) = trimmed.split_once('=') {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn format_command_output(output: &crate::exec::CommandOutput) -> String {
    let mut message = format!("Command exited with code {}", output.exit_code);
    if !output.stdout.trim().is_empty() {
        message.push_str("\nSTDOUT:\n");
        message.push_str(output.stdout.trim());
    }
    if !output.stderr.trim().is_empty() {
        message.push_str("\nSTDERR:\n");
        message.push_str(output.stderr.trim());
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::{CommandOutput, CommandSpec};
    use std::cell::RefCell;

    struct MockRunner {
        outputs: RefCell<Vec<CommandOutput>>,
    }

    impl Runner for MockRunner {
        fn run(&self, _spec: &CommandSpec) -> std::io::Result<CommandOutput> {
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    #[test]
    fn extract_app_path_from_build_settings_works() {
        let stdout = "\
Build settings for action build and target App:
    BUILT_PRODUCTS_DIR = /tmp/DerivedData/Build/Products/Debug-iphonesimulator
    FULL_PRODUCT_NAME = App.app
";
        let path = extract_app_path_from_build_settings(stdout).expect("path");
        assert_eq!(
            path,
            "/tmp/DerivedData/Build/Products/Debug-iphonesimulator/App.app"
        );
    }

    #[test]
    fn read_bundle_id_uses_defaults_when_available() {
        let runner = MockRunner {
            outputs: RefCell::new(vec![CommandOutput {
                exit_code: 0,
                terminated_by_signal: false,
                stdout: "com.example.App".to_string(),
                stderr: String::new(),
            }]),
        };

        let result = read_bundle_id("/tmp/Test.app", &runner).expect("bundle id");
        assert_eq!(result, "com.example.App");
    }
}
