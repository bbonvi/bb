#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("bookmark not found")]
    NotFound,

    #[error("bookmark with this url already exists at id {0}")]
    AlreadyExists(u64),

    #[error("reqwest error: {0:?}")]
    Reqwest(#[from] reqwest::Error),

    #[error("io error: {0:?}")]
    IO(#[from] std::io::Error),

    #[error("Base64: {0:?}")]
    Base64(#[from] base64::DecodeError),

    #[error("unexpected error: {0:?}")]
    Other(#[from] anyhow::Error),
}
