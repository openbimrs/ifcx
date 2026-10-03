use std::collections::HashMap;
use std::convert::Infallible;

use super::LayerResolver;
use crate::json::WriteError;
use crate::model::IfcxFile;

/// Serves layers held in memory, keyed by the exact import `uri`, like
/// upstream's `InMemoryLayerProvider`.
#[derive(Debug, Clone, Default)]
pub struct MemoryResolver {
    layers: HashMap<String, Vec<u8>>,
}

impl MemoryResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Serves `bytes` for `uri`, replacing and returning any earlier entry.
    pub fn insert(&mut self, uri: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Option<Vec<u8>> {
        self.layers.insert(uri.into(), bytes.into())
    }

    /// Serves `file` under its `header.id`, as upstream's in-memory provider
    /// does. An `integrity` check on such a layer hashes the JSON this crate
    /// writes, not any original bytes.
    pub fn insert_file(&mut self, file: &IfcxFile) -> Result<Option<Vec<u8>>, WriteError> {
        Ok(self.insert(file.header.id.clone(), file.to_json_string()?))
    }
}

impl LayerResolver for MemoryResolver {
    type Error = Infallible;

    fn load(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.layers.get(key).cloned())
    }
}
