//! Storage boundary for product jobs, projections, immutable artifacts, and rights metadata.

pub trait ArtifactStore {
    type Error;

    fn put_if_absent(&mut self, content_hash: &str, bytes: &[u8]) -> Result<(), Self::Error>;
    fn get(&self, content_hash: &str) -> Result<Option<Vec<u8>>, Self::Error>;
}
