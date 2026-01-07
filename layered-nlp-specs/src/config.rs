//! Pipeline configuration.

/// Configuration for the spec test pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Resolver names to run.
    pub resolvers: Vec<String>,
}

impl PipelineConfig {
    /// Standard contract analysis pipeline.
    pub fn standard() -> Self {
        Self {
            resolvers: vec!["standard".into()],
        }
    }

    /// Create with specific resolvers.
    pub fn with_resolvers(resolvers: Vec<String>) -> Self {
        Self { resolvers }
    }
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self::standard()
    }
}
