// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

//! Message types for Zello protocol

use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};

/// Messages that can be sent to Zello
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum Message {
    /// Logon request
    #[serde(rename = "logon")]
    Logon {
        seq: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        username: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        password: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        auth_token: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        channels: Option<Vec<String>>,
    },

    /// Send text message
    #[serde(rename = "send_text_message")]
    SendTextMessage {
        seq: u32,
        channel: String,
        #[serde(rename = "for")]
        for_user: Option<String>,
        text: String,
    },

    /// Start outgoing audio stream
    #[serde(rename = "start_stream")]
    StartStream {
        seq: u32,
        channel: String,
        #[serde(rename = "for")]
        for_user: Option<String>,
        codec: String,
        codec_header: Option<String>,
        packet_duration: u32,
    },

    /// Stop outgoing audio stream
    #[serde(rename = "stop_stream")]
    StopStream { seq: u32, stream_id: u32 },
}

impl Message {
    /// Create a logon message with username/password/token
    #[must_use]
    pub fn logon_password(
        seq: u32,
        username: String,
        password: String,
        auth_token: String,
        channel: String,
    ) -> Self {
        Self::Logon {
            seq,
            username: Some(username),
            password: Some(password),
            auth_token: Some(auth_token),
            channels: Some(vec![channel]),
        }
    }

    /// Create a logon message with token only
    #[must_use]
    pub fn logon_token(seq: u32, auth_token: String, channel: String) -> Self {
        Self::Logon {
            seq,
            username: None,
            password: None,
            auth_token: Some(auth_token),
            channels: Some(vec![channel]),
        }
    }

    /// Create a text message
    #[must_use]
    pub fn send_text(seq: u32, channel: String, text: String) -> Self {
        Self::SendTextMessage {
            seq,
            channel,
            for_user: None,
            text,
        }
    }

    /// Create a text message for a specific callsign
    #[must_use]
    pub fn send_text_for_callsign(
        seq: u32,
        channel: String,
        text: String,
        for_user: String,
    ) -> Self {
        Self::SendTextMessage {
            seq,
            channel,
            for_user: Some(for_user),
            text,
        }
    }

    /// Create a start stream message
    #[must_use]
    pub fn start_stream(seq: u32, channel: String, codec: String, packet_duration: u32) -> Self {
        Self::StartStream {
            seq,
            channel,
            for_user: None,
            codec,
            codec_header: None,
            packet_duration,
        }
    }

    /// Create a stop stream message
    #[must_use]
    pub fn stop_stream(seq: u32, stream_id: u32) -> Self {
        Self::StopStream { seq, stream_id }
    }

    /// Get the sequence number if present
    #[must_use]
    pub fn seq(&self) -> Option<u32> {
        match self {
            Self::Logon { seq, .. }
            | Self::SendTextMessage { seq, .. }
            | Self::StartStream { seq, .. }
            | Self::StopStream { seq, .. } => Some(*seq),
        }
    }
}

/// Response messages from Zello
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    /// Logon response
    Logon {
        seq: u32,
        success: bool,
        refresh_token: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Generic response
    Generic {
        seq: u32,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

impl Response {
    /// Get the sequence number if present
    #[must_use]
    pub fn seq(&self) -> Option<u32> {
        match self {
            Self::Logon { seq, .. } | Self::Generic { seq, .. } => Some(*seq),
        }
    }

    /// Check if this is a success response
    #[must_use]
    pub fn is_success(&self) -> bool {
        match self {
            Self::Logon { success, .. } | Self::Generic { success, .. } => *success,
        }
    }

    /// Get error message if present
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Logon { error, .. } | Self::Generic { error, .. } => error.as_deref(),
        }
    }
}

/// Error messages from Zello
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum Error {
    /// Error message
    #[serde(rename = "on_error")]
    Error { error: String },
}

impl Error {
    /// Get the error message
    #[must_use]
    pub fn error(&self) -> &str {
        match self {
            Self::Error { error } => error,
        }
    }
}

/// Events that can be received from Zello
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum Event {
    /// Text message received
    #[serde(rename = "on_text_message")]
    TextMessage {
        message_id: u64,
        channel: String,
        from: String,
        #[serde(rename = "for")]
        for_user: Option<String>,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        author: Option<String>,
    },

    /// Audio stream start
    #[serde(rename = "on_stream_start")]
    AudioStart {
        stream_id: u32,
        channel: String,
        from: String,
        #[serde(rename = "for")]
        for_user: Option<String>,
        codec: String,
        codec_header: Option<String>,
        packet_duration: u32,
    },

    /// Audio data packet
    #[serde(rename = "on_stream_data")]
    AudioData {
        stream_id: u32,
        packet_id: u32,
        data: Vec<u8>,
    },

    /// Audio stream stop
    #[serde(rename = "on_stream_stop")]
    AudioStop { stream_id: u32 },

    /// Channel status update
    #[serde(rename = "on_channel_status")]
    ChannelStatus {
        channel: String,
        status: String,
        users_online: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        images: Option<Vec<ChannelImage>>,
    },

    /// User online/offline status
    #[serde(rename = "on_online_status")]
    OnlineStatus {
        channel: String,
        from: String,
        online: bool,
    },
}

/// Top-level enum for all incoming messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IncomingMessage {
    Response(Response),
    Error(Error),
    Event(Event),
}

/// Channel image information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelImage {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
}

/// Opus codec header with audio parameters
#[derive(Debug, Clone)]
pub struct CodecHeader {
    pub sample_rate_hz: u16,
    pub frames_per_packet: u8,
    pub frame_size_ms: u8,
}

impl CodecHeader {
    /// Decode a base64-encoded codec header
    ///
    /// # Errors
    ///
    /// Returns an error if the base64 decoding fails or the header length is invalid
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let bytes = STANDARD.decode(encoded)?;
        Self::from_bytes(Bytes::from(bytes))
    }

    /// Parse codec header from bytes
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes length is invalid
    pub fn from_bytes(mut bytes: Bytes) -> Result<Self> {
        if bytes.len() != 4 {
            return Err(anyhow!(
                "Invalid codec header length: expected 4 bytes, got {}",
                bytes.len()
            ));
        }

        let sample_rate_hz = bytes.get_u16_le();
        let frames_per_packet = bytes.get_u8();
        let frame_size_ms = bytes.get_u8();

        Ok(Self {
            sample_rate_hz,
            frames_per_packet,
            frame_size_ms,
        })
    }

    /// Encode to base64 string
    #[must_use]
    pub fn to_base64(&self) -> String {
        STANDARD.encode(self.to_bytes())
    }

    /// Convert to bytes
    #[must_use]
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_u16_le(self.sample_rate_hz);
        buf.put_u8(self.frames_per_packet);
        buf.put_u8(self.frame_size_ms);
        buf.freeze()
    }
}

impl Default for CodecHeader {
    fn default() -> Self {
        Self {
            sample_rate_hz: 16000,
            frames_per_packet: 1,
            frame_size_ms: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::send_text(1, "test_channel".to_string(), "Hello".to_string());
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        assert!(json.contains("send_text_message"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_message_seq() {
        let msg = Message::send_text(42, "channel".to_string(), "test".to_string());
        assert_eq!(msg.seq(), Some(42));
    }
}
