use xcbuild_bridge::build_for_testing_sim::{
    BuildForTestingSimParamsInput, build_for_testing_sim_command, build_for_testing_sim_from_input,
};
use xcbuild_bridge::build_sim::{BuildSimParamsInput, build_sim_command, build_sim_from_input};
use xcbuild_bridge::session::{SessionDefaults, SessionSetDefaultsParams, SessionStore};
use xcbuild_bridge::smoke_sim::{SmokeSimParamsInput, smoke_sim_from_input};
use xcbuild_bridge::test_sim::{TestSimParamsInput, test_sim_command, test_sim_from_input};
use xcbuild_bridge::test_without_building_sim::{
    TestWithoutBuildingSimParamsInput, test_without_building_sim_command,
    test_without_building_sim_from_input,
};

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

#[test]
fn build_for_testing_requires_required_fields() {
    let defaults = SessionDefaults::default();
    let input = BuildForTestingSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: Some("App".to_string()),
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        test_plan: None,
    };

    let err = build_for_testing_sim_from_input(input, &defaults).unwrap_err();
    assert!(
        err.contains("projectPath")
            || err.contains("workspacePath")
            || err.contains("simulatorId")
            || err.contains("simulatorName")
    );
}

#[test]
fn build_for_testing_command_has_action() {
    let defaults = SessionDefaults {
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: Some("App".to_string()),
        simulator_id: Some("SIM-UUID".to_string()),
        ..SessionDefaults::default()
    };
    let input = BuildForTestingSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        test_plan: Some("SmokePlan".to_string()),
    };

    let params = build_for_testing_sim_from_input(input, &defaults).unwrap();
    let spec = build_for_testing_sim_command(&params);
    assert_eq!(spec.args.last(), Some(&"build-for-testing".to_string()));
    assert!(spec.args.iter().any(|arg| arg == "-testPlan"));
    assert!(spec.args.iter().any(|arg| arg == "SmokePlan"));
}

#[test]
fn test_sim_requires_required_fields() {
    let defaults = SessionDefaults::default();
    let input = TestSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: Some("App".to_string()),
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        only_testing: None,
        skip_testing: None,
        test_runner_env: None,
    };

    let err = test_sim_from_input(input, &defaults).unwrap_err();
    assert!(
        err.contains("projectPath")
            || err.contains("workspacePath")
            || err.contains("simulatorId")
            || err.contains("simulatorName")
    );
}

#[test]
fn test_sim_merges_defaults_and_sets_fallbacks() {
    let defaults = SessionDefaults {
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: Some("App".to_string()),
        simulator_id: Some("SIM-UUID".to_string()),
        ..SessionDefaults::default()
    };
    let input = TestSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        only_testing: None,
        skip_testing: None,
        test_runner_env: None,
    };

    let params = test_sim_from_input(input, &defaults).unwrap();
    assert_eq!(params.configuration, "Debug");
    assert!(params.use_latest_os);
}

#[test]
fn test_sim_command_includes_only_and_skip_testing() {
    let defaults = SessionDefaults {
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: Some("App".to_string()),
        simulator_id: Some("SIM-UUID".to_string()),
        ..SessionDefaults::default()
    };
    let input = TestSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        only_testing: Some(vec!["UITests/SmokeTests/testLaunch".to_string()]),
        skip_testing: Some(vec!["UITests/SmokeTests/testSlow".to_string()]),
        test_runner_env: None,
    };

    let params = test_sim_from_input(input, &defaults).unwrap();
    let spec = test_sim_command(&params);
    assert!(spec.args.iter().any(|arg| arg == "-only-testing"));
    assert!(
        spec.args
            .iter()
            .any(|arg| arg == "UITests/SmokeTests/testLaunch")
    );
    assert!(spec.args.iter().any(|arg| arg == "-skip-testing"));
    assert!(
        spec.args
            .iter()
            .any(|arg| arg == "UITests/SmokeTests/testSlow")
    );
    assert_eq!(spec.args.last(), Some(&"test".to_string()));
}

#[test]
fn test_without_building_requires_scheme_when_no_xctestrun() {
    let defaults = SessionDefaults::default();
    let input = TestWithoutBuildingSimParamsInput {
        project_path: Some("App.xcodeproj".to_string()),
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: Some("SIM-UUID".to_string()),
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        only_testing: None,
        skip_testing: None,
        test_plan: None,
        xctestrun: None,
        test_runner_env: None,
    };

    let err = test_without_building_sim_from_input(input, &defaults).unwrap_err();
    assert!(err.contains("scheme"));
}

#[test]
fn test_without_building_allows_xctestrun_without_scheme() {
    let defaults = SessionDefaults::default();
    let input = TestWithoutBuildingSimParamsInput {
        project_path: Some("App.xcodeproj".to_string()),
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: Some("SIM-UUID".to_string()),
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        only_testing: None,
        skip_testing: None,
        test_plan: None,
        xctestrun: Some("AppTests.xctestrun".to_string()),
        test_runner_env: None,
    };

    let params = test_without_building_sim_from_input(input, &defaults).unwrap();
    let spec = test_without_building_sim_command(&params, None, false);
    assert!(spec.args.iter().any(|arg| arg == "-xctestrun"));
    assert!(spec.args.iter().any(|arg| arg == "AppTests.xctestrun"));
}

#[test]
fn smoke_sim_requires_scheme_when_building() {
    let defaults = SessionDefaults::default();
    let input = SmokeSimParamsInput {
        project_path: Some("App.xcodeproj".to_string()),
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: Some("SIM-UUID".to_string()),
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        only_testing: None,
        skip_testing: None,
        test_plan: None,
        xctestrun: None,
        test_runner_env: None,
        boot_sim: None,
        wait_for_boot: None,
        skip_build: Some(false),
    };

    let err = smoke_sim_from_input(input, &defaults).unwrap_err();
    assert!(err.contains("scheme"));
}

#[test]
fn smoke_sim_allows_skip_build_with_xctestrun() {
    let defaults = SessionDefaults::default();
    let input = SmokeSimParamsInput {
        project_path: Some("App.xcodeproj".to_string()),
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: Some("SIM-UUID".to_string()),
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        only_testing: None,
        skip_testing: None,
        test_plan: None,
        xctestrun: Some("AppTests.xctestrun".to_string()),
        test_runner_env: None,
        boot_sim: None,
        wait_for_boot: None,
        skip_build: Some(true),
    };

    let params = smoke_sim_from_input(input, &defaults).unwrap();
    assert!(params.skip_build);
}

#[test]
fn test_sim_command_prefixes_test_runner_env() {
    let defaults = SessionDefaults {
        workspace_path: Some("App.xcworkspace".to_string()),
        scheme: Some("App".to_string()),
        simulator_id: Some("SIM-UUID".to_string()),
        ..SessionDefaults::default()
    };
    let mut env = std::collections::HashMap::new();
    env.insert("FOO".to_string(), "bar".to_string());
    env.insert("TEST_RUNNER_BAZ".to_string(), "qux".to_string());
    let input = TestSimParamsInput {
        project_path: None,
        workspace_path: None,
        scheme: None,
        configuration: None,
        simulator_id: None,
        simulator_name: None,
        derived_data_path: None,
        extra_args: None,
        use_latest_os: None,
        only_testing: None,
        skip_testing: None,
        test_runner_env: Some(env),
    };

    let params = test_sim_from_input(input, &defaults).unwrap();
    let spec = test_sim_command(&params);
    let env = spec.env.expect("env should be set");
    assert_eq!(env.get("TEST_RUNNER_FOO"), Some(&"bar".to_string()));
    assert_eq!(env.get("TEST_RUNNER_BAZ"), Some(&"qux".to_string()));
    assert!(!env.contains_key("FOO"));
}
