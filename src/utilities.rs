// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

//! Utility functions for Zello client operations

use std::collections::VecDeque;
use std::sync::Arc;

use crate::{
    CPAL_BUFFER_SIZE, CPAL_CHANNELS, CPAL_SAMPLE_RATE, CPAL_VECTOR_QUEUE_CAPACITY, OPUS_CHANNELS,
    OPUS_SAMPLE_RATE, PCM_I16_TO_F32,
};
use crate::{Credentials, ZelloClient, ZelloConfig};
use anyhow::{Result, anyhow};
use audiopus::coder::Decoder;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use crossbeam_channel::Receiver;
use dotenvy::{dotenv, from_path};
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

/// Initialize logging with environment filter
pub fn initialize_logging() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    Ok(())
}

/// Load environment variables from .env file
///
/// # Errors
///
/// Returns an error if the required .env file is not found or cannot be loaded
pub fn load_dotenv() -> Result<()> {
    dotenv().map_err(|e| anyhow!("Warning: Could not load .env file {e}"))?;
    Ok(())
}

/// Load environment variables from a specific file
///
/// # Errors
///
/// Returns an error if the required file is not found or cannot be loaded
pub fn load_dotenv_from_file(path: &str) -> Result<()> {
    from_path(path).map_err(|e| anyhow!("Warning: Could not load '{path}' file {e}"))?;
    Ok(())
}

/// Load Zello credentials from environment variables
///
/// # Errors
///
/// Returns an error if required environment variables are not set
pub fn load_credentials() -> Result<Credentials> {
    let username = std::env::var("ZELLO_USERNAME")
        .map_err(|_| anyhow!("Please set ZELLO_USERNAME environment variable"))?;

    let password = std::env::var("ZELLO_PASSWORD")
        .map_err(|_| anyhow!("Please set ZELLO_PASSWORD environment variable"))?;

    let token = std::env::var("ZELLO_TOKEN")
        .map_err(|_| anyhow!("Please set ZELLO_TOKEN environment variable"))?;

    let channel = std::env::var("ZELLO_CHANNEL")
        .map_err(|_| anyhow!("Please set ZELLO_CHANNEL environment variable"))?;

    Ok(Credentials {
        username,
        password,
        token,
        channel,
    })
}

/// Create an Opus audio decoder
///
/// # Errors
///
/// Returns an error if decoder creation fails
pub fn create_decoder() -> Result<Arc<Mutex<Decoder>>> {
    let decoder = Decoder::new(OPUS_SAMPLE_RATE, OPUS_CHANNELS)?;
    Ok(Arc::new(Mutex::new(decoder)))
}

/// Get the default audio output device
///
/// # Errors
///
/// Returns an error if no output device is found
pub fn get_audio_device() -> Result<Device> {
    let host = cpal::default_host();
    host.default_output_device()
        .ok_or_else(|| anyhow!("No output device found"))
}

/// Create audio stream configuration
#[must_use]
pub fn create_stream_config() -> StreamConfig {
    StreamConfig {
        channels: CPAL_CHANNELS,
        sample_rate: CPAL_SAMPLE_RATE,
        buffer_size: CPAL_BUFFER_SIZE,
    }
}

/// Setup audio output stream
///
/// # Errors
///
/// Returns an error if stream creation or playback fails
pub fn setup_audio_output(pcm_rx: Arc<Mutex<Receiver<Vec<i16>>>>) -> Result<Stream> {
    let device = get_audio_device()?;
    let stream_config = create_stream_config();

    let mut buffer = VecDeque::<f32>::with_capacity(CPAL_VECTOR_QUEUE_CAPACITY);
    let err_fn = |err| error!("Stream error: {err:?}");

    let stream = device.build_output_stream(
        &stream_config,
        move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
            process_audio_output(output, &pcm_rx, &mut buffer);
        },
        err_fn,
        None,
    )?;

    stream.play()?;
    Ok(stream)
}

/// Process audio output by filling the output buffer
pub fn process_audio_output(
    output: &mut [f32],
    pcm_rx: &Arc<Mutex<Receiver<Vec<i16>>>>,
    buffer: &mut VecDeque<f32>,
) {
    // Try to refill buffer from channel
    if let Ok(rx) = pcm_rx.try_lock() {
        while let Ok(pcm) = rx.try_recv() {
            debug!("🎤 Received PCM chunk: {} samples", pcm.len());
            for &sample in &pcm {
                buffer.push_back(f32::from(sample) * PCM_I16_TO_F32);
            }
        }
    }

    debug!(
        "🎤 Output buffer size: {}, internal buffer: {}",
        output.len(),
        buffer.len()
    );

    // Fill output from buffer
    for out in output.iter_mut() {
        *out = buffer.pop_front().unwrap_or(0.0);
    }
}

/// Connect to Zello and authenticate
///
/// # Errors
///
/// Returns an error if connection or authentication fails
pub async fn connect_to_zello(credentials: &Credentials) -> Result<ZelloClient> {
    info!("Connecting to Zello...");
    info!("Username: {}", credentials.username);
    info!("Channel: {}", credentials.channel);

    let config = ZelloConfig::new(
        credentials.username.clone(),
        credentials.password.clone(),
        credentials.token.clone(),
        credentials.channel.clone(),
    );

    match ZelloClient::new(config).await {
        Ok(client) => {
            info!("✓ Connected and authenticated successfully!");
            Ok(client)
        }
        Err(e) => Err(anyhow!("✗ Failed to connect or authenticate: {e}")),
    }
}
