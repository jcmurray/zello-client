// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

#![warn(clippy::all, clippy::pedantic, rust_2018_idioms)]
#![warn(missing_debug_implementations, clippy::unwrap_used)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod error;
pub mod handlers;
pub mod message;
pub mod protocol;
pub mod utilities;

// Re-exports for convenience
use audiopus::{Channels, SampleRate};
pub use client::*;
pub use error::{Result, ZelloError};
pub use handlers::{handle_message, process_audio_output};
pub use message::{CodecHeader, Error, Event, IncomingMessage, Message, Response};
pub use protocol::Protocol;
pub use utilities::{
    connect_to_zello, create_decoder, initialize_logging, load_credentials, load_dotenv,
    setup_audio_output,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GIT version
pub const GIT_VERSION: &str = env!("GIT_VERSION");

/// Zello uses a mono audio stream.
pub const OPUS_CHANNELS: Channels = Channels::Mono;

/// Zello uses a 16kHz sample rate.
pub const OPUS_SAMPLE_RATE: SampleRate = SampleRate::Hz16000;

/// PCM buffer capacity for Zello audio stream.
pub const PCM_CHANNEL_CAPACITY: usize = 20;

/// PCM buffer size for Zello audio stream.
pub const PCM_BUFFER_SIZE: usize = 1920;

///Cpal sample rate to match Zello audio stream.
pub const CPAL_SAMPLE_RATE: cpal::SampleRate = cpal::SampleRate(16000);

///Cpal channels to match Zello audio stream.
pub const CPAL_CHANNELS: u16 = 1;

///Cpal vector queue capacity to match Zello audio stream.
pub const CPAL_VECTOR_QUEUE_CAPACITY: usize = 8192;

/// PCM to cpal conversion factor for PCM to cpal audio stream.
pub const PCM_I16_TO_F32: f32 = 1.0 / 32768.0;

/// Cpal buffer size to match Zello audio stream.
#[allow(clippy::cast_possible_truncation)]
pub const CPAL_BUFFER_SIZE: cpal::BufferSize =
    cpal::BufferSize::Fixed(PCM_BUFFER_SIZE as cpal::FrameCount);

/// Default Zello WebSocket URL
pub const ZELLO_DEFAULT_URL: &str = "wss://zello.io/ws";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_git_version() {
        assert!(!GIT_VERSION.is_empty());
    }
}
