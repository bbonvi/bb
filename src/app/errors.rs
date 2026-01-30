#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("io error: {0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    Base64(#[from] base64::DecodeError),

    #[error("{0:#}")]
    Other(#[from] anyhow::Error),
}
