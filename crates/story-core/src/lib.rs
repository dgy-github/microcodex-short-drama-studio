//! Product-owned short-drama job and artifact contracts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenreMode {
    Auto,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryFormat {
    pub episodes: u16,
    pub minutes_per_episode: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryBudget {
    pub max_tokens: u64,
    pub max_cny_fen: u64,
    pub deadline_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryJob {
    pub schema: String,
    pub job_id: String,
    pub input: String,
    pub genre_mode: GenreMode,
    pub allowed_genres: Vec<String>,
    pub audience: String,
    pub format: StoryFormat,
    pub content_limits: Vec<String>,
    pub budget: StoryBudget,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("job schema must be story-job/v1")]
    Schema,
    #[error("job id and input must not be blank")]
    Blank,
    #[error("episode count and duration must be positive")]
    Format,
}

impl StoryJob {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema != "story-job/v1" {
            return Err(ValidationError::Schema);
        }
        if self.job_id.trim().is_empty() || self.input.trim().is_empty() {
            return Err(ValidationError::Blank);
        }
        if self.format.episodes == 0 || self.format.minutes_per_episode == 0 {
            return Err(ValidationError::Format);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> StoryJob {
        StoryJob {
            schema: "story-job/v1".into(),
            job_id: "job_1".into(),
            input: "母亲卖掉老房子后，三个成年子女第一次回家吃饭。".into(),
            genre_mode: GenreMode::Auto,
            allowed_genres: vec!["family".into()],
            audience: "25-45".into(),
            format: StoryFormat {
                episodes: 8,
                minutes_per_episode: 2,
            },
            content_limits: vec![],
            budget: StoryBudget {
                max_tokens: 100_000,
                max_cny_fen: 1_000,
                deadline_seconds: 600,
            },
        }
    }

    #[test]
    fn valid_job_passes() {
        assert_eq!(job().validate(), Ok(()));
    }

    #[test]
    fn blank_input_fails() {
        let mut value = job();
        value.input = " ".into();
        assert_eq!(value.validate(), Err(ValidationError::Blank));
    }
}
