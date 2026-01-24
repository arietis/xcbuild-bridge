use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDefaults {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub simulator_name: Option<String>,
    pub simulator_id: Option<String>,
    pub device_id: Option<String>,
    pub use_latest_os: Option<bool>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    defaults: SessionDefaults,
}

impl SessionStore {
    pub fn new() -> Self {
        SessionStore {
            defaults: SessionDefaults::default(),
        }
    }

    pub fn get_all(&self) -> SessionDefaults {
        self.defaults.clone()
    }

    pub fn set_defaults(&mut self, mut params: SessionSetDefaultsParams) -> SessionSetResult {
        let mut notices = Vec::new();

        if params.project_path.is_some() && params.workspace_path.is_some() {
            params.project_path = None;
            notices.push(
                "Both projectPath and workspacePath were provided; keeping workspacePath and ignoring projectPath.".to_string(),
            );
        }

        if params.simulator_id.is_some() && params.simulator_name.is_some() {
            params.simulator_name = None;
            notices.push(
                "Both simulatorId and simulatorName were provided; keeping simulatorId and ignoring simulatorName.".to_string(),
            );
        }

        if params.project_path.is_some() {
            if self.defaults.workspace_path.is_some() {
                notices.push("Cleared workspacePath because projectPath was set.".to_string());
            }
            self.defaults.workspace_path = None;
        }

        if params.workspace_path.is_some() {
            if self.defaults.project_path.is_some() {
                notices.push("Cleared projectPath because workspacePath was set.".to_string());
            }
            self.defaults.project_path = None;
        }

        if params.simulator_id.is_some() {
            if self.defaults.simulator_name.is_some() {
                notices.push("Cleared simulatorName because simulatorId was set.".to_string());
            }
            self.defaults.simulator_name = None;
        }

        if params.simulator_name.is_some() {
            if self.defaults.simulator_id.is_some() {
                notices.push("Cleared simulatorId because simulatorName was set.".to_string());
            }
            self.defaults.simulator_id = None;
        }

        if let Some(value) = params.project_path {
            self.defaults.project_path = Some(value);
        }
        if let Some(value) = params.workspace_path {
            self.defaults.workspace_path = Some(value);
        }
        if let Some(value) = params.scheme {
            self.defaults.scheme = Some(value);
        }
        if let Some(value) = params.configuration {
            self.defaults.configuration = Some(value);
        }
        if let Some(value) = params.simulator_name {
            self.defaults.simulator_name = Some(value);
        }
        if let Some(value) = params.simulator_id {
            self.defaults.simulator_id = Some(value);
        }
        if let Some(value) = params.device_id {
            self.defaults.device_id = Some(value);
        }
        if let Some(value) = params.use_latest_os {
            self.defaults.use_latest_os = Some(value);
        }
        if let Some(value) = params.arch {
            self.defaults.arch = Some(value);
        }

        SessionSetResult {
            updated: self.get_all(),
            notices,
        }
    }

    pub fn clear_all(&mut self) {
        self.defaults = SessionDefaults::default();
    }

    pub fn clear_keys(&mut self, keys: &[SessionKey]) {
        for key in keys {
            match key {
                SessionKey::ProjectPath => self.defaults.project_path = None,
                SessionKey::WorkspacePath => self.defaults.workspace_path = None,
                SessionKey::Scheme => self.defaults.scheme = None,
                SessionKey::Configuration => self.defaults.configuration = None,
                SessionKey::SimulatorName => self.defaults.simulator_name = None,
                SessionKey::SimulatorId => self.defaults.simulator_id = None,
                SessionKey::DeviceId => self.defaults.device_id = None,
                SessionKey::UseLatestOs => self.defaults.use_latest_os = None,
                SessionKey::Arch => self.defaults.arch = None,
            }
        }
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSetDefaultsParams {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub simulator_name: Option<String>,
    pub simulator_id: Option<String>,
    pub device_id: Option<String>,
    pub use_latest_os: Option<bool>,
    pub arch: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSetResult {
    pub updated: SessionDefaults,
    pub notices: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionClearDefaultsParams {
    pub keys: Option<Vec<SessionKey>>,
    pub all: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum SessionKey {
    #[serde(rename = "projectPath")]
    ProjectPath,
    #[serde(rename = "workspacePath")]
    WorkspacePath,
    #[serde(rename = "scheme")]
    Scheme,
    #[serde(rename = "configuration")]
    Configuration,
    #[serde(rename = "simulatorName")]
    SimulatorName,
    #[serde(rename = "simulatorId")]
    SimulatorId,
    #[serde(rename = "deviceId")]
    DeviceId,
    #[serde(rename = "useLatestOS")]
    UseLatestOs,
    #[serde(rename = "arch")]
    Arch,
}
