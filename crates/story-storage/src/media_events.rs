//! Durable append-only event log for image and video project runs.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRunEvent {
    pub schema: String,
    pub seq: u64,
    pub project_id: String,
    pub run_id: String,
    pub request_id: String,
    pub event_type: String,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum MediaEventStoreError {
    #[error("media event identity is invalid")]
    InvalidIdentity,
    #[error("media event log lock is unavailable")]
    Lock,
    #[error("media event log is corrupt")]
    Corrupt,
    #[error("media event io failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("media event encoding failed: {0}")]
    Encoding(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct MediaEventStore {
    path: PathBuf,
    state: Arc<Mutex<EventState>>,
}

struct EventState {
    events: Vec<MediaRunEvent>,
    accepted: HashMap<String, usize>,
    terminal: HashMap<String, usize>,
}

impl EventState {
    fn from_events(events: Vec<MediaRunEvent>) -> Result<Self, MediaEventStoreError> {
        let mut state = Self {
            events: Vec::new(),
            accepted: HashMap::new(),
            terminal: HashMap::new(),
        };
        for event in events {
            state.push(event)?;
        }
        Ok(state)
    }

    fn next_seq(&self) -> u64 {
        self.events
            .last()
            .map_or(1, |event| event.seq.saturating_add(1))
    }

    fn push(&mut self, event: MediaRunEvent) -> Result<(), MediaEventStoreError> {
        let index = self.events.len();
        if event.seq != self.next_seq() {
            return Err(MediaEventStoreError::Corrupt);
        }
        if event.event_type == "run.accepted"
            && self.accepted.insert(event.run_id.clone(), index).is_some()
        {
            return Err(MediaEventStoreError::Corrupt);
        }
        if matches!(
            event.event_type.as_str(),
            "run.completed" | "run.failed" | "run.cancelled"
        ) && self.terminal.insert(event.run_id.clone(), index).is_some()
        {
            return Err(MediaEventStoreError::Corrupt);
        }
        self.events.push(event);
        Ok(())
    }
}

fn read_events(path: &PathBuf) -> Result<Vec<MediaRunEvent>, MediaEventStoreError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let mut ids = HashSet::new();
    text.lines()
        .map(|line| {
            let event: MediaRunEvent =
                serde_json::from_str(line).map_err(|_| MediaEventStoreError::Corrupt)?;
            if event.schema != "media-run-event/v1"
                || !ids.insert(event.seq)
                || !valid_id(&event.project_id)
                || !valid_id(&event.run_id)
                || !valid_id(&event.request_id)
                || !valid_id(&event.event_type)
            {
                return Err(MediaEventStoreError::Corrupt);
            }
            Ok(event)
        })
        .collect()
}

impl MediaEventStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, MediaEventStoreError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let events = read_events(&path)?;
        let state = EventState::from_events(events)?;
        Ok(Self {
            path,
            state: Arc::new(Mutex::new(state)),
        })
    }

    pub fn append(
        &self,
        project_id: &str,
        run_id: &str,
        request_id: &str,
        event_type: &str,
        payload: Value,
    ) -> Result<MediaRunEvent, MediaEventStoreError> {
        if [project_id, run_id, request_id, event_type]
            .iter()
            .any(|value| !valid_id(value))
        {
            return Err(MediaEventStoreError::InvalidIdentity);
        }
        let mut state = self.state.lock().map_err(|_| MediaEventStoreError::Lock)?;
        let event = MediaRunEvent {
            schema: "media-run-event/v1".into(),
            seq: state.next_seq(),
            project_id: project_id.into(),
            run_id: run_id.into(),
            request_id: request_id.into(),
            event_type: event_type.into(),
            payload,
        };
        self.write_event(&event)?;
        state.push(event.clone())?;
        Ok(event)
    }

    pub fn replay(&self, after_seq: u64) -> Result<Vec<MediaRunEvent>, MediaEventStoreError> {
        let state = self.state.lock().map_err(|_| MediaEventStoreError::Lock)?;
        Ok(state
            .events
            .iter()
            .filter(|event| event.seq > after_seq)
            .cloned()
            .collect())
    }

    pub fn append_terminal(
        &self,
        project_id: &str,
        run_id: &str,
        request_id: &str,
        event_type: &str,
        payload: Value,
    ) -> Result<(MediaRunEvent, bool), MediaEventStoreError> {
        if !matches!(event_type, "run.completed" | "run.failed" | "run.cancelled")
            || [project_id, run_id, request_id]
                .iter()
                .any(|value| !valid_id(value))
        {
            return Err(MediaEventStoreError::InvalidIdentity);
        }
        let mut state = self.state.lock().map_err(|_| MediaEventStoreError::Lock)?;
        if let Some(index) = state.terminal.get(run_id) {
            return Ok((state.events[*index].clone(), false));
        }
        let event = MediaRunEvent {
            schema: "media-run-event/v1".into(),
            seq: state.next_seq(),
            project_id: project_id.into(),
            run_id: run_id.into(),
            request_id: request_id.into(),
            event_type: event_type.into(),
            payload,
        };
        self.write_event(&event)?;
        state.push(event.clone())?;
        Ok((event, true))
    }

    pub fn append_acceptance(
        &self,
        project_id: &str,
        run_id: &str,
        request_id: &str,
        payload: Value,
    ) -> Result<(MediaRunEvent, bool), MediaEventStoreError> {
        if [project_id, run_id, request_id]
            .iter()
            .any(|value| !valid_id(value))
        {
            return Err(MediaEventStoreError::InvalidIdentity);
        }
        let mut state = self.state.lock().map_err(|_| MediaEventStoreError::Lock)?;
        if let Some(index) = state.accepted.get(run_id) {
            return Ok((state.events[*index].clone(), false));
        }
        let event = MediaRunEvent {
            schema: "media-run-event/v1".into(),
            seq: state.next_seq(),
            project_id: project_id.into(),
            run_id: run_id.into(),
            request_id: request_id.into(),
            event_type: "run.accepted".into(),
            payload,
        };
        self.write_event(&event)?;
        state.push(event.clone())?;
        Ok((event, true))
    }

    fn write_event(&self, event: &MediaRunEvent) -> Result<(), MediaEventStoreError> {
        let mut bytes = serde_json::to_vec(event)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(())
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_durable_ordered_and_replayed_after_cursor() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("events.jsonl");
        let store = MediaEventStore::open(&path).unwrap();
        store
            .append(
                "project_1",
                "media_run_1",
                "img_a",
                "run.accepted",
                Value::Null,
            )
            .unwrap();
        store
            .append(
                "project_1",
                "media_run_1",
                "img_a",
                "run.completed",
                Value::Null,
            )
            .unwrap();
        let reopened = MediaEventStore::open(&path).unwrap();
        let tail = reopened.replay(1).unwrap();
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0].seq, 2);
        assert_eq!(tail[0].event_type, "run.completed");
    }

    #[test]
    fn many_runs_reopen_with_indexes_and_terminal_arbitration_intact() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("events.jsonl");
        let store = MediaEventStore::open(&path).unwrap();
        for index in 0..250 {
            let run_id = format!("media_run_{index}");
            let request_id = format!("img_{index}");
            assert!(
                store
                    .append_acceptance("project_1", &run_id, &request_id, Value::Null)
                    .unwrap()
                    .1
            );
            assert!(
                store
                    .append_terminal(
                        "project_1",
                        &run_id,
                        &request_id,
                        "run.completed",
                        Value::Null,
                    )
                    .unwrap()
                    .1
            );
        }
        let reopened = MediaEventStore::open(&path).unwrap();
        assert_eq!(reopened.replay(0).unwrap().len(), 500);
        assert!(
            !reopened
                .append_terminal(
                    "project_1",
                    "media_run_249",
                    "img_249",
                    "run.failed",
                    Value::Null,
                )
                .unwrap()
                .1
        );
        assert_eq!(reopened.replay(499).unwrap().len(), 1);
    }
}
