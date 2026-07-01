use thiserror::Error;

pub type AgResult<T> = Result<T, AgError>;

#[derive(Debug, Error)]
pub enum AgError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported input format: {0}")]
    UnsupportedFormat(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("invalid table: {0}")]
    InvalidTable(String),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("processing cancelled")]
    Cancelled,
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
}
