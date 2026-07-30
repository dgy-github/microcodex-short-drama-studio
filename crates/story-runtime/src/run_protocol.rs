use crate::GenreContext;
use serde::{Deserialize, Serialize};
use story_core::StoryJob;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyKey(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IdempotencyKeyError {
    #[error("idempotency key must contain 16-128 printable ASCII characters without spaces")]
    Invalid,
}

impl IdempotencyKey {
    pub fn new(value: impl Into<String>) -> Result<Self, IdempotencyKeyError> {
        let value = value.into();
        if !(16..=128).contains(&value.len())
            || value.bytes().any(|byte| !(0x21..=0x7e).contains(&byte))
        {
            return Err(IdempotencyKeyError::Invalid);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandAcceptance {
    pub schema: String,
    pub command: String,
    pub request_id: String,
    pub job_id: String,
    pub run_id: String,
    pub event_stream_url: String,
    pub accepted_event_seq: u64,
    pub status: String,
}

#[derive(Serialize)]
pub(crate) struct StartRunRequest<'a> {
    schema: &'static str,
    job: &'a StoryJob,
    #[serde(skip_serializing_if = "Option::is_none")]
    genre_context: Option<&'a GenreContext>,
}

impl<'a> StartRunRequest<'a> {
    pub(crate) fn new(job: &'a StoryJob) -> Self {
        Self {
            schema: "start-run-command/v1",
            job,
            genre_context: None,
        }
    }

    pub(crate) fn with_genre_context(job: &'a StoryJob, genre_context: &'a GenreContext) -> Self {
        Self {
            schema: "start-run-command/v1",
            job,
            genre_context: Some(genre_context),
        }
    }
}

pub(crate) fn valid_acceptance(acceptance: &CommandAcceptance, job_id: &str) -> bool {
    acceptance.schema == "story-command-acceptance/v1"
        && acceptance.command == "StartRun"
        && acceptance.status == "accepted"
        && acceptance.job_id == job_id
        && !acceptance.request_id.is_empty()
        && acceptance.run_id.starts_with("run_")
        && acceptance.accepted_event_seq > 0
        && acceptance.event_stream_url == format!("/v1/runs/{}/events", acceptance.run_id)
}

pub(crate) fn frame_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|window| window == b"\n\n")
}

pub(crate) fn sse_data(frame: &[u8]) -> Option<&[u8]> {
    frame
        .split(|byte| *byte == b'\n')
        .find_map(|line| line.strip_prefix(b"data: "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotency_key_is_bounded_and_header_safe() {
        assert!(IdempotencyKey::new("short").is_err());
        assert!(IdempotencyKey::new("contains a space and is long enough").is_err());
        let key = IdempotencyKey::new("start-run-key-00000001").unwrap();
        assert_eq!(key.as_str(), "start-run-key-00000001");
    }

    #[test]
    fn sse_frame_helpers_find_data_and_replay_boundary() {
        let bytes = b"id: 1\nevent: run.accepted\ndata: {\"seq\":1}\n\n: replay-complete\n\n";
        let first_end = frame_end(bytes).unwrap();
        assert_eq!(sse_data(&bytes[..first_end + 2]), Some(&b"{\"seq\":1}"[..]));
        assert!(bytes[first_end + 2..].starts_with(b": replay-complete"));
    }
}
