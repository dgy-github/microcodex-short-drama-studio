//! Asynchronous command and durable event protocol shared by Rust and Campaign.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const EVENT_PROTOCOL: &str = "story-agent-event/v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub protocol: String,
    pub event_id: String,
    pub seq: u64,
    pub occurred_at: String,
    pub causation_id: String,
    pub correlation_id: String,
    pub job_id: String,
    pub run_id: String,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub event_type: String,
    pub schema_version: u16,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EventError {
    #[error("event protocol must be story-agent-event/v1")]
    Protocol,
    #[error("durable event sequence must be positive")]
    Sequence,
    #[error("event identity and type fields must not be blank")]
    Identity,
}

impl EventEnvelope {
    pub fn validate(&self) -> Result<(), EventError> {
        if self.protocol != EVENT_PROTOCOL {
            return Err(EventError::Protocol);
        }
        if self.seq == 0 {
            return Err(EventError::Sequence);
        }
        let fields = [&self.event_id, &self.job_id, &self.run_id, &self.event_type];
        if fields.iter().any(|field| field.trim().is_empty()) {
            return Err(EventError::Identity);
        }
        Ok(())
    }

    pub fn deduplication_key(&self) -> (&str, u64) {
        (&self.run_id, self.seq)
    }
}

#[derive(Debug, Default)]
pub struct SequenceCursor {
    run_id: Option<String>,
    last_seq: u64,
}

impl SequenceCursor {
    pub fn accept(&mut self, event: &EventEnvelope) -> bool {
        if self.run_id.as_deref() != Some(event.run_id.as_str()) {
            self.run_id = Some(event.run_id.clone());
            self.last_seq = 0;
        }
        if event.seq <= self.last_seq {
            return false;
        }
        self.last_seq = event.seq;
        true
    }

    pub fn last_event_id(&self) -> Option<u64> {
        (self.last_seq > 0).then_some(self.last_seq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(seq: u64) -> EventEnvelope {
        EventEnvelope {
            protocol: EVENT_PROTOCOL.into(),
            event_id: format!("evt_{seq}"),
            seq,
            occurred_at: "2026-07-26T12:00:00Z".into(),
            causation_id: "cmd_1".into(),
            correlation_id: "req_1".into(),
            job_id: "job_1".into(),
            run_id: "run_1".into(),
            task_id: Some("t01".into()),
            agent_id: Some("story-coordinator".into()),
            event_type: "task.started".into(),
            schema_version: 1,
            payload: Value::Null,
        }
    }

    #[test]
    fn duplicate_sequence_is_ignored() {
        let mut cursor = SequenceCursor::default();
        assert!(cursor.accept(&event(1)));
        assert!(!cursor.accept(&event(1)));
        assert_eq!(cursor.last_event_id(), Some(1));
    }

    #[test]
    fn invalid_zero_sequence_fails() {
        assert_eq!(event(0).validate(), Err(EventError::Sequence));
    }
}
