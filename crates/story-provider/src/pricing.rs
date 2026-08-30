use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const SCHEMA: &str = "provider-pricing-catalog/v1";
const TOKENS_PER_MILLION: u128 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingCatalog {
    schema: String,
    catalog_id: String,
    effective_at: String,
    entries: Vec<PricingEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct PricingEntry {
    provider: String,
    model: String,
    prompt_cny_fen_per_million_tokens: u64,
    completion_cny_fen_per_million_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PricingQuote {
    pub catalog_id: String,
    pub provider: String,
    pub model: String,
    pub cost_cny_fen: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PricingError {
    #[error("pricing catalog is invalid")]
    InvalidCatalog,
    #[error("provider model has no configured price")]
    UnknownRoute,
    #[error("provider cost calculation overflowed")]
    Overflow,
}

impl PricingCatalog {
    pub fn from_json(input: &str) -> Result<Self, PricingError> {
        let catalog: Self =
            serde_json::from_str(input).map_err(|_| PricingError::InvalidCatalog)?;
        if catalog.schema != SCHEMA
            || catalog.catalog_id.trim().is_empty()
            || catalog.effective_at.trim().is_empty()
            || catalog.entries.is_empty()
        {
            return Err(PricingError::InvalidCatalog);
        }
        let mut routes = HashSet::new();
        for entry in &catalog.entries {
            if !valid_provider(&entry.provider)
                || entry.model.trim().is_empty()
                || entry.model.trim() != entry.model
                || entry.model.len() > 128
                || !routes.insert((entry.provider.as_str(), entry.model.as_str()))
            {
                return Err(PricingError::InvalidCatalog);
            }
        }
        Ok(catalog)
    }

    pub fn quote(
        &self,
        provider: &str,
        model: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> Result<PricingQuote, PricingError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.provider == provider && entry.model == model)
            .ok_or(PricingError::UnknownRoute)?;
        let prompt = u128::from(prompt_tokens)
            .checked_mul(u128::from(entry.prompt_cny_fen_per_million_tokens))
            .ok_or(PricingError::Overflow)?;
        let completion = u128::from(completion_tokens)
            .checked_mul(u128::from(entry.completion_cny_fen_per_million_tokens))
            .ok_or(PricingError::Overflow)?;
        let numerator = prompt
            .checked_add(completion)
            .ok_or(PricingError::Overflow)?;
        let cost = numerator
            .checked_add(TOKENS_PER_MILLION - 1)
            .ok_or(PricingError::Overflow)?
            / TOKENS_PER_MILLION;
        Ok(PricingQuote {
            catalog_id: self.catalog_id.clone(),
            provider: provider.into(),
            model: model.into(),
            cost_cny_fen: u64::try_from(cost).map_err(|_| PricingError::Overflow)?,
        })
    }
}

fn valid_provider(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> PricingCatalog {
        PricingCatalog::from_json(
            r#"{
                "schema":"provider-pricing-catalog/v1",
                "catalog_id":"fixture-2026-01",
                "effective_at":"2026-01-01T00:00:00Z",
                "entries":[{
                    "provider":"fixture_provider",
                    "model":"fixture-model",
                    "prompt_cny_fen_per_million_tokens":100,
                    "completion_cny_fen_per_million_tokens":200
                }]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn exact_route_is_quoted_and_fractional_fen_rounds_up() {
        let quote = catalog()
            .quote("fixture_provider", "fixture-model", 5_000, 2_500)
            .unwrap();
        assert_eq!(quote.cost_cny_fen, 1);
        let quote = catalog()
            .quote("fixture_provider", "fixture-model", 1_000_000, 1_000_000)
            .unwrap();
        assert_eq!(quote.cost_cny_fen, 300);
    }

    #[test]
    fn unknown_or_ambiguous_routes_fail_closed() {
        assert_eq!(
            catalog().quote("fixture_provider", "other", 1, 1),
            Err(PricingError::UnknownRoute)
        );
        let duplicate = r#"{
            "schema":"provider-pricing-catalog/v1","catalog_id":"x","effective_at":"x",
            "entries":[
                {"provider":"p","model":"m","prompt_cny_fen_per_million_tokens":1,"completion_cny_fen_per_million_tokens":1},
                {"provider":"p","model":"m","prompt_cny_fen_per_million_tokens":2,"completion_cny_fen_per_million_tokens":2}
            ]
        }"#;
        assert_eq!(
            PricingCatalog::from_json(duplicate),
            Err(PricingError::InvalidCatalog)
        );
    }
}
