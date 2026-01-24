use xcbuild_bridge::build_sim::{BuildSimParamsInput, build_sim_command, build_sim_from_input};
use xcbuild_bridge::session::{SessionDefaults, SessionSetDefaultsParams, SessionStore};

#[test]
fn session_set_defaults_prefers_workspace() {
    let mut store = SessionStore::new();
    let result = store.set_defaults(SessionSetDefaultsParams {
        project_path: Some("App.xcodeproj".to_string()),
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: None,
        configuration: None,
        simulator_name: None,
        simulator_id: None,
        device_id: None,
        use_latest_os: None,
        arch: None,
    });

    assert_eq!(
        result.updated.workspace_path,
        Some("App.xcworkspace".to_string())
    );
    assert_eq!(result.updated.project_path, None);
    assert!(!result.notices.is_empty());
}

#[test]
fn session_set_defaults_clears_conflicting_keys() {
    let mut store = SessionStore::new();
    store.set_defaults(SessionSetDefaultsParams {
        project_path: None,
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: None,
        configuration: None,
        simulator_name: Some("iPhone 16".to_string()),
        simulator_id: None,
        device_id: None,
        use_latest_os: None,
        arch: None,
    });

    store.set_defaults(SessionSetDefaultsParams {
        project_path: Some("App.xcodeproj".to_string()),
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_name: None,
        simulator_id: Some("SIM-UUID".to_string()),
        device_id: None,
        use_latest_os: None,
        arch: None,
    });

    let updated = store.get_all();
    assert_eq!(updated.workspace_path, None);
    assert_eq!(updated.project_path, Some("App.xcodeproj".to_string()));
    assert_eq!(updated.simulator_name, None);
    assert_eq!(updated.simulator_id, Some("SIM-UUID".to_string()));
}

#[test]
fn build_sim_requires_required_fields() {
    let defaults = SessionDefaults::default();
    let input = BuildSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: Some("App".to_string()),
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        prefer_xcodebuild: None,
    };

    let err = build_sim_from_input(input, &defaults).unwrap_err();
    assert!(
        err.contains("projectPath")
            || err.contains("workspacePath")
            || err.contains("simulatorId")
            || err.contains("simulatorName")
    );
}

#[test]
fn build_sim_merges_defaults_and_sets_fallbacks() {
    let defaults = SessionDefaults {
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: Some("App".to_string()),
        simulator_id: Some("SIM-UUID".to_string()),
        ..SessionDefaults::default()
    };
    let input = BuildSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        prefer_xcodebuild: None,
    };

    let params = build_sim_from_input(input, &defaults).unwrap();
    assert_eq!(params.configuration, "Debug");
    assert!(params.use_latest_os);
    assert!(!params.prefer_xcodebuild);
}

#[test]
fn build_sim_command_uses_simulator_id() {
    let defaults = SessionDefaults {
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: Some("App".to_string()),
        simulator_id: Some("SIM-UUID".to_string()),
        ..SessionDefaults::default()
    };
    let input = BuildSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        prefer_xcodebuild: None,
    };

    let params = build_sim_from_input(input, &defaults).unwrap();
    let spec = build_sim_command(&params);
    assert!(spec.args.iter().any(|arg| arg == "id=SIM-UUID"));
}

#[test]
fn build_sim_command_uses_simulator_name_and_latest() {
    let defaults = SessionDefaults {
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: Some("App".to_string()),
        simulator_name: Some("iPhone 16".to_string()),
        ..SessionDefaults::default()
    };
    let input = BuildSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        prefer_xcodebuild: None,
    };

    let params = build_sim_from_input(input, &defaults).unwrap();
    let spec = build_sim_command(&params);
    assert!(
        spec.args
            .iter()
            .any(|arg| arg.contains("platform=iOS Simulator,name=iPhone 16,OS=latest"))
    );
}
