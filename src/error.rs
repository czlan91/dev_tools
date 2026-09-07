#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    AssetLoad(#[from] std::io::Error),
}
