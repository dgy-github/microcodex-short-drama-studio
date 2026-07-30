use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use story_core::{ContentForm, StoryJob};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenreRetrievalSource {
    pub source_id: String,
    pub license_id: String,
    pub content_sha256: String,
    pub usage: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenreContext {
    pub schema: String,
    pub pack_id: String,
    pub constraint_profile_id: String,
    pub genre: String,
    pub architect_directives: Vec<String>,
    pub reviewer_directives: Vec<String>,
    pub human_writing: HumanWritingContext,
    pub retrieval_sources: Vec<GenreRetrievalSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanWritingContext {
    pub profile_id: String,
    pub task_directives: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GenrePackOption {
    pub pack_id: String,
    pub display_name: String,
    pub genre: String,
    pub default_audience: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GenrePackError {
    #[error("genre pack registry configuration is invalid")]
    InvalidConfig,
    #[error("genre pack selection is incomplete")]
    IncompleteSelection,
    #[error("genre pack or constraint profile does not exist")]
    UnknownSelection,
    #[error("story job violates the selected genre pack")]
    ConstraintViolation,
}

pub struct GenrePackRegistry {
    packs: HashMap<String, PackDocument>,
    profiles: HashMap<String, ConstraintDocument>,
    agents: HashMap<String, AgentDocument>,
    retrieval: HashMap<String, RetrievalDocument>,
    human_writing: HumanWritingDocument,
}

#[derive(Deserialize)]
struct RegistryDocument {
    schema: String,
    human_writing_profile: String,
    packs: Vec<String>,
    constraint_profiles: Vec<String>,
    agent_profiles: Vec<String>,
    retrieval_collections: Vec<String>,
}

#[derive(Deserialize)]
struct HumanWritingDocument {
    schema: String,
    profile_id: String,
    task_directives: HashMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct PackDocument {
    template_id: String,
    display_name: String,
    genre: String,
    content_form: ContentForm,
    constraints: PackConstraints,
    constraint_profiles: Vec<String>,
    agent_configuration: AgentBinding,
    retrieval_collections: Vec<String>,
}

#[derive(Deserialize)]
struct PackConstraints {
    audience: Option<String>,
}

#[derive(Deserialize)]
struct AgentBinding {
    architect_profile: String,
    reviewer_profiles: Vec<String>,
}

#[derive(Deserialize)]
struct ConstraintDocument {
    profile_id: String,
    content_form: ContentForm,
    episodes_range: [u16; 2],
    minutes_per_episode_range: [u16; 2],
}

#[derive(Deserialize)]
struct AgentDocument {
    profile_id: String,
    role: String,
    genre: String,
    system_directives: Vec<String>,
}

#[derive(Deserialize)]
struct RetrievalDocument {
    collection_id: String,
    sources: Vec<RetrievalSourceDocument>,
}

#[derive(Deserialize)]
struct RetrievalSourceDocument {
    source_id: String,
    content_sha256: String,
    license_id: String,
    allowed_uses: Vec<String>,
}

impl GenrePackRegistry {
    pub fn load(repository_root: &Path) -> Result<Self, GenrePackError> {
        if !repository_root.is_absolute() {
            return Err(GenrePackError::InvalidConfig);
        }
        let config_root = repository_root.join("config");
        let registry: RegistryDocument =
            read_json(&config_root.join("genre-packs/registry-v1.json"))?;
        if registry.schema != "genre-pack-registry/v1" {
            return Err(GenrePackError::InvalidConfig);
        }
        let human_writing: HumanWritingDocument =
            read_json(&config_root.join(&registry.human_writing_profile))?;
        let expected_tasks = ["t07", "t10", "t12", "t15", "t16"];
        if human_writing.schema != "human-writing-profile/v1"
            || human_writing.profile_id.is_empty()
            || human_writing.task_directives.len() != expected_tasks.len()
            || expected_tasks.iter().any(|task_id| {
                human_writing
                    .task_directives
                    .get(*task_id)
                    .is_none_or(|directives| {
                        directives.is_empty()
                            || directives
                                .iter()
                                .any(|directive| directive.trim().is_empty())
                    })
            })
        {
            return Err(GenrePackError::InvalidConfig);
        }
        Ok(Self {
            packs: load_map(&config_root, registry.packs, |document: &PackDocument| {
                &document.template_id
            })?,
            profiles: load_map(
                &config_root,
                registry.constraint_profiles,
                |document: &ConstraintDocument| &document.profile_id,
            )?,
            agents: load_map(
                &config_root,
                registry.agent_profiles,
                |document: &AgentDocument| &document.profile_id,
            )?,
            retrieval: load_map(
                &config_root,
                registry.retrieval_collections,
                |document: &RetrievalDocument| &document.collection_id,
            )?,
            human_writing,
        })
    }

    pub fn resolve_job(&self, job: &StoryJob) -> Result<Option<GenreContext>, GenrePackError> {
        let (pack_id, profile_id) = match (&job.genre_pack_id, &job.constraint_profile_id) {
            (None, None) => return Ok(None),
            (Some(pack_id), Some(profile_id)) => (pack_id, profile_id),
            _ => return Err(GenrePackError::IncompleteSelection),
        };
        let pack = self
            .packs
            .get(pack_id)
            .ok_or(GenrePackError::UnknownSelection)?;
        let profile = self
            .profiles
            .get(profile_id)
            .ok_or(GenrePackError::UnknownSelection)?;
        if pack.content_form != job.content_form()
            || profile.content_form != job.content_form()
            || !pack.constraint_profiles.contains(profile_id)
            || !job.allowed_genres.iter().any(|genre| genre == &pack.genre)
            || !(profile.episodes_range[0]..=profile.episodes_range[1])
                .contains(&job.format.episodes)
            || !(profile.minutes_per_episode_range[0]..=profile.minutes_per_episode_range[1])
                .contains(&job.format.minutes_per_episode)
        {
            return Err(GenrePackError::ConstraintViolation);
        }
        let architect = self
            .agents
            .get(&pack.agent_configuration.architect_profile)
            .filter(|agent| agent.role == "architect" && agent.genre == pack.genre)
            .ok_or(GenrePackError::InvalidConfig)?;
        let reviewers = pack
            .agent_configuration
            .reviewer_profiles
            .iter()
            .map(|profile| {
                self.agents
                    .get(profile)
                    .filter(|agent| agent.role == "reviewer" && agent.genre == pack.genre)
                    .ok_or(GenrePackError::InvalidConfig)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let retrieval_sources = pack
            .retrieval_collections
            .iter()
            .map(|collection| {
                self.retrieval
                    .get(collection)
                    .ok_or(GenrePackError::InvalidConfig)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flat_map(|collection| &collection.sources)
            .map(|source| {
                if !source.allowed_uses.iter().any(|usage| usage == "retrieval") {
                    return Err(GenrePackError::InvalidConfig);
                }
                Ok(GenreRetrievalSource {
                    source_id: source.source_id.clone(),
                    license_id: source.license_id.clone(),
                    content_sha256: source.content_sha256.clone(),
                    usage: "genre_pack_guidance".into(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(GenreContext {
            schema: "genre-context/v1".into(),
            pack_id: pack.template_id.clone(),
            constraint_profile_id: profile.profile_id.clone(),
            genre: pack.genre.clone(),
            architect_directives: architect.system_directives.clone(),
            reviewer_directives: reviewers
                .into_iter()
                .flat_map(|agent| agent.system_directives.clone())
                .collect(),
            human_writing: HumanWritingContext {
                profile_id: self.human_writing.profile_id.clone(),
                task_directives: self.human_writing.task_directives.clone(),
            },
            retrieval_sources,
        }))
    }

    pub fn options(&self) -> Vec<GenrePackOption> {
        let mut options = self
            .packs
            .values()
            .map(|pack| GenrePackOption {
                pack_id: pack.template_id.clone(),
                display_name: pack.display_name.clone(),
                genre: pack.genre.clone(),
                default_audience: pack
                    .constraints
                    .audience
                    .clone()
                    .unwrap_or_else(|| "18-45".into()),
            })
            .collect::<Vec<_>>();
        options.sort_by(|left, right| left.pack_id.cmp(&right.pack_id));
        options
    }
}

fn load_map<T: for<'de> Deserialize<'de>>(
    config_root: &Path,
    references: Vec<String>,
    id: impl Fn(&T) -> &String,
) -> Result<HashMap<String, T>, GenrePackError> {
    let mut values = HashMap::new();
    for reference in references {
        let path = safe_config_path(config_root, &reference)?;
        let value: T = read_json(&path)?;
        let identifier = id(&value).clone();
        if values.insert(identifier, value).is_some() {
            return Err(GenrePackError::InvalidConfig);
        }
    }
    Ok(values)
}

fn safe_config_path(root: &Path, reference: &str) -> Result<PathBuf, GenrePackError> {
    if reference.is_empty()
        || reference.contains('\\')
        || reference.contains(':')
        || reference
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(GenrePackError::InvalidConfig);
    }
    let path = root.join(reference);
    if !path.is_file() {
        return Err(GenrePackError::InvalidConfig);
    }
    Ok(path)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, GenrePackError> {
    let bytes = std::fs::read(path).map_err(|_| GenrePackError::InvalidConfig)?;
    serde_json::from_slice(&bytes).map_err(|_| GenrePackError::InvalidConfig)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(episodes: u16, pack: &str, profile: &str, genre: &str) -> StoryJob {
        serde_json::from_value(serde_json::json!({
            "schema": "story-job/v1",
            "job_id": "job_genre_pack",
            "content_form": "scripted_short_drama",
            "input": "一名维修工必须在开门前救出被困电梯的人。",
            "genre_mode": "fixed",
            "allowed_genres": [genre],
            "genre_pack_id": pack,
            "constraint_profile_id": profile,
            "audience": "25-45",
            "format": {"episodes": episodes, "minutes_per_episode": 2},
            "content_limits": [],
            "budget": {"max_tokens": 1000, "max_cny_fen": 100, "deadline_seconds": 60}
        }))
        .unwrap()
    }

    #[test]
    fn config_only_packs_resolve_short_and_long_shared_contracts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = GenrePackRegistry::load(&root).unwrap();
        let short = registry
            .resolve_job(&job(8, "family-grounded-v1", "short-vertical-v1", "family"))
            .unwrap()
            .unwrap();
        let long = registry
            .resolve_job(&job(
                60,
                "suspense-closed-room-v1",
                "long-serial-v1",
                "suspense",
            ))
            .unwrap()
            .unwrap();
        assert_eq!(short.genre, "family");
        assert_eq!(long.genre, "suspense");
        assert!(!long.reviewer_directives.is_empty());
        assert!(!short.retrieval_sources.is_empty());
        assert_eq!(
            short.human_writing.profile_id,
            "short-drama-human-writing-v1"
        );
        assert_eq!(short.human_writing.task_directives.len(), 5);
        for task_id in ["t07", "t10", "t12", "t15", "t16"] {
            assert!(short
                .human_writing
                .task_directives
                .get(task_id)
                .is_some_and(|directives| !directives.is_empty()));
        }
    }

    #[test]
    fn pack_constraints_reject_mismatched_episode_count() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = GenrePackRegistry::load(&root).unwrap();
        assert!(matches!(
            registry.resolve_job(&job(6, "family-grounded-v1", "long-serial-v1", "family")),
            Err(GenrePackError::ConstraintViolation)
        ));
    }

    #[test]
    fn registry_projects_all_desktop_genre_options() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = GenrePackRegistry::load(&root).unwrap();
        let options = registry.options();
        assert_eq!(options.len(), 8);
        for (pack_id, genre) in [
            ("family-grounded-v1", "family"),
            ("suspense-closed-room-v1", "suspense"),
            ("urban-romance-grounded-v1", "urban_romance"),
            ("revenge-earned-turnaround-v1", "revenge"),
            ("workplace-credible-growth-v1", "workplace"),
            ("rural-community-v1", "rural"),
            ("comedy-situational-v1", "comedy"),
            ("historical-causal-v1", "historical"),
        ] {
            assert!(options
                .iter()
                .any(|option| option.pack_id == pack_id && option.genre == genre));
        }
    }
}
