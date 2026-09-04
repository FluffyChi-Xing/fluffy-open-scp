use dbpf::ResourceId;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("dbpf error: {0}")]
    Dbpf(#[from] dbpf::Error),
    #[error("property parse error: {0}")]
    Properties(#[from] sc_properties::Error),
    #[error("registry error: {0}")]
    Registry(#[from] sc_registry::Error),
    #[error("resource {0} not found in package")]
    ResourceNotFound(ResourceId),
}
