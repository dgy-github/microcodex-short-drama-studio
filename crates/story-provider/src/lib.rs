//! Trusted provider boundary. Concrete network clients arrive after protocol M0.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

pub trait StructuredTextProvider {
    type Error;

    fn generate(&self, schema: &str, prompt: &str) -> Result<(String, Usage), Self::Error>;
}
