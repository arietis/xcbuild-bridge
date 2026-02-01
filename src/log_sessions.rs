use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct LogSession {
    pub id: String,
    pub processes: Vec<Child>,
    pub file_path: PathBuf,
    pub simulator_id: String,
    pub bundle_id: String,
}

#[derive(Debug, Default)]
pub struct LogSessionStore {
    sessions: HashMap<String, LogSession>,
    counter: u64,
}

impl LogSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            counter: 0,
        }
    }

    pub fn insert(
        &mut self,
        processes: Vec<Child>,
        file_path: PathBuf,
        simulator_id: String,
        bundle_id: String,
    ) -> String {
        let id = self.next_id();
        let session = LogSession {
            id: id.clone(),
            processes,
            file_path,
            simulator_id,
            bundle_id,
        };
        self.sessions.insert(id.clone(), session);
        id
    }

    pub fn remove(&mut self, session_id: &str) -> Option<LogSession> {
        self.sessions.remove(session_id)
    }

    fn next_id(&mut self) -> String {
        self.counter += 1;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("log-{}-{}", self.counter, nanos)
    }
}
