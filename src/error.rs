// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

//! Error types for the Zello client

use thiserror::Error;

/// Result type alias for Zello operations
pub type Result<T> = anyhow::Result<T, ZelloError>;

/// Error types that can occur when using the Zello client
#[derive(Debug, Error)]
pub enum ZelloError {
    /// WebSocket connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Authentication failed
    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    /// Invalid message format or protocol error
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),

    /// Network I/O error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// WebSocket error (boxed to reduce size)
    #[error("WebSocket error: {0}")]
    WebSocketError(#[from] Box<tokio_tungstenite::tungstenite::Error>),

    /// Audio codec error
    #[error("Audio error: {0}")]
    AudioError(String),

    /// Client not connected
    #[error("Client is not connected")]
    NotConnected,

    /// Invalid configuration
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Operation timeout
    #[error("Operation timed out")]
    Timeout,

    /// Channel error
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Unknown or unexpected error
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<tokio_tungstenite::tungstenite::Error> for ZelloError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocketError(Box::new(err))
    }
}

impl From<Box<dyn std::error::Error>> for ZelloError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        ZelloError::Other(format!("{err}").into())
    }
}
