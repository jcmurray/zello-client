// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

//! Handler functions for Zello client operations

use std::collections::VecDeque;
use std::sync::Arc;

use crate::{CodecHeader, Error, Event, IncomingMessage, Response, ZelloClient};
use crate::{OPUS_CHANNELS, PCM_BUFFER_SIZE, PCM_I16_TO_F32};
use anyhow::Result;
use audiopus::{Channels, MutSignals, coder::Decoder, packet::Packet};
use crossbeam_channel::{Receiver, Sender};
use tokio::sync::Mutex;
use tracing::{Level, debug, error, info, level_enabled, warn};

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

/// Handle incoming message from Zello
pub async fn handle_message(
    client: &mut ZelloClient,
    message: IncomingMessage,
    decoder: Arc<Mutex<Decoder>>,
    pcm_tx: &Sender<Vec<i16>>,
) {
    match message {
        IncomingMessage::Event(Event::TextMessage {
            from,
            text,
            author,
            channel,
            ..
        }) => {
            handle_text_message(&from, &text, author.as_deref(), &channel);
        }

        IncomingMessage::Event(Event::AudioStart {
            stream_id,
            from,
            codec,
            codec_header,
            channel,
            packet_duration,
            ..
        }) => {
            if let Err(e) = handle_audio_start(
                client,
                stream_id,
                from,
                codec,
                codec_header.as_deref(),
                channel,
                Some(packet_duration),
            ) {
                warn!("Failed to handle audio start: {}", e);
            }
        }

        IncomingMessage::Event(Event::AudioStop { stream_id }) => {
            if let Err(e) = handle_audio_stop(client, stream_id) {
                warn!("Failed to handle audio stop: {}", e);
            }
        }

        IncomingMessage::Event(Event::AudioData {
            stream_id,
            packet_id,
            data,
        }) => {
            handle_audio_data(stream_id, packet_id, data, decoder, pcm_tx).await;
        }

        IncomingMessage::Event(Event::OnlineStatus {
            channel,
            from,
            online,
        }) => {
            handle_online_status(&channel, &from, online);
        }

        IncomingMessage::Event(Event::ChannelStatus {
            channel,
            status,
            users_online,
            ..
        }) => {
            handle_channel_status(&channel, &status, users_online);
        }

        IncomingMessage::Error(Error::Error { error }) => {
            error!("❌ Error: {error}");
        }

        IncomingMessage::Response(
            Response::Generic {
                seq,
                success,
                error,
            }
            | Response::Logon {
                seq,
                success,
                error,
                ..
            },
        ) => {
            handle_response(seq, success, error.as_deref());
        }
    }
}

/// Handle text message event
pub fn handle_text_message(from: &str, text: &str, author: Option<&str>, channel: &str) {
    let display_name = author.unwrap_or(from);
    info!("[{channel}] {display_name}: {text}");
}

/// Handle audio stream start event
///
/// #Errors
///
/// Returns an error if fail to start audio stream
pub fn handle_audio_start(
    client: &mut ZelloClient,
    stream_id: u32,
    from: String,
    codec: String,
    codec_header: Option<&str>,
    channel: String,
    packet_duration: Option<u32>,
) -> Result<()> {
    let header = match codec_header {
        Some(h) => CodecHeader::from_base64(h)?,
        None => CodecHeader::default(),
    };

    let (rate, frames, size) = (
        header.sample_rate_hz,
        header.frames_per_packet,
        header.frame_size_ms,
    );

    if level_enabled!(Level::DEBUG) {
        debug!(
            "[{channel}] 🎤 {from} started speaking (stream: {stream_id}, codec: {codec}, \
             rate: {rate}, frames: {frames}, size: {size}, duration: {packet_duration:?})"
        );
    } else {
        info!("[{channel}] 🎤 {from} started speaking on stream {stream_id}");
    }

    client.add_inbound_stream(stream_id, channel, codec, Some(from))?;

    Ok(())
}

/// Handle audio stream stop event
///
/// #Errors
///
/// Returns an error if fail to stop audio stream
pub fn handle_audio_stop(client: &mut ZelloClient, stream_id: u32) -> Result<()> {
    if let Some(stream_info) = client.get_inbound_stream(stream_id) {
        info!(
            "[{}] 🎤 {} stopped speaking on stream {stream_id}",
            stream_info.channel,
            stream_info.callsign.as_deref().unwrap_or("unknown")
        );
    }
    client.remove_inbound_stream(stream_id)?;

    Ok(())
}

/// Handle audio data packet
pub async fn handle_audio_data(
    stream_id: u32,
    packet_id: u32,
    data: Vec<u8>,
    decoder: Arc<Mutex<Decoder>>,
    pcm_tx: &Sender<Vec<i16>>,
) {
    debug!("🎤 Audio data {stream_id} {packet_id}");

    let channel_count = match OPUS_CHANNELS {
        Channels::Mono | Channels::Auto => 1,
        Channels::Stereo => 2,
    };

    let mut decoder = decoder.lock().await;
    let mut pcm_buf = vec![0i16; PCM_BUFFER_SIZE];

    let packet = match Packet::try_from(&data) {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to parse audio packet: {e}");
            return;
        }
    };

    let output = match MutSignals::try_from(&mut pcm_buf) {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to create MutSignals: {e}");
            return;
        }
    };

    if let Ok(samples) = decoder.decode(Some(packet), output, false) {
        let total_samples = samples * channel_count;
        let _ = pcm_tx.try_send(pcm_buf[..total_samples].to_vec());
    }
}

/// Handle online status event
pub fn handle_online_status(channel: &str, from: &str, online: bool) {
    let status = if online { "online" } else { "offline" };
    info!("[{channel}] {from} is now {status}");
}

/// Handle channel status event
pub fn handle_channel_status(channel: &str, status: &str, users_online: u32) {
    info!("[{channel}] Status: {status} ({users_online} users online)");
}

/// Handle response message
pub fn handle_response(seq: u32, success: bool, error: Option<&str>) {
    if success {
        info!("✓ Response to #{seq}: Success");
    } else {
        error!(
            "✗ Response to #{seq}: Failed - {}",
            error.unwrap_or("Unknown error")
        );
    }
}
