#![warn(rust_2018_idioms)]
#![allow(unused_imports)]
#![allow(clippy::blacklisted_name)]

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Kubernetes API Error: {0}")]
    KubeError(#[source] kube::Error),

    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("VaultError: {0}")]
    VaultError(#[from] hashicorp_vault::Error),

    #[error("ArcanumError: {reason:?}")]
    ArcanumError { reason: String },

    #[error("CryptoError: {0}")]
    CryptoError(#[from] ecies_ed25519::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// state machinery for Kubernetes, exposed to actix
pub mod manager;

pub use manager::Manager;

/// generated resource type (for crdgen)
pub use manager::SyncedSecret;

/// controller configuration
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct AppConfig {
    version: u8,
    host: String,
    token: String,
    path: String,
}
