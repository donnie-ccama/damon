//! Core types and traits for the cortado session manager.

pub mod bridge;
pub mod config;
pub mod entity;
pub mod memory;
pub mod models;
pub mod session_name;
pub mod slug;
pub mod store;

use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid TOML at {path}: {msg}")]
    Toml { path: PathBuf, msg: String },
    #[error("not found: {0}")]
    NotFound(String),
    #[error("ambiguous reference {0:?}: matches {1}")]
    Ambiguous(String, String),
    #[error("already exists: {0}")]
    Exists(String),
    #[error("{0}")]
    Invalid(String),
    #[error("cannot resolve the home directory (is $HOME set?)")]
    NoHome,
}
