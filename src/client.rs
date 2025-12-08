// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

//! Zello client implementation

use crate::error::{Result, ZelloError};
use crate::handlers::handle_message;
use crate::message::IncomingMessage;
use crate::message::Message;
use crate::message::Response;
use crate::protocol::Protocol;
use audiopus::coder::Decoder;
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};
use tracing::{debug, error, info};

/// Configuration for Zello client
#[derive(Debug, Clone)]
pub struct ZelloConfig {
    /// Username for authentication
    pub username: Option<String>,
    /// Password for authentication
    pub password: Option<String>,
    /// Channel to join
    pub channel: String,
    /// Optional authentication token (alternative to username/password)
    pub auth_token: Option<String>,
}

impl ZelloConfig {
    /// Create a new configuration with username, password and token
    #[must_use]
    pub fn new(username: String, password: String, auth_token: String, channel: String) -> Self {
        Self {
            username: Some(username),
            password: Some(password),
            channel,
            auth_token: Some(auth_token),
        }
    }

    /// Validate the configuration
    ///
    /// #Errors
    ///
    /// Returns an error if the configuration is invalid
    pub fn validate(&self) -> Result<()> {
        if self.channel.is_empty() {
            return Err(ZelloError::ConfigError(
                "Channel cannot be empty".to_string(),
            ));
        }

        if !(self.auth_token.is_some() && self.username.is_some() && self.password.is_some()) {
            return Err(ZelloError::ConfigError(
                "Must provide auth_token and username/password".to_string(),
            ));
        }

        if let (Some(username), Some(password), Some(token)) =
            (&self.username, &self.password, &self.auth_token)
            && (username.is_empty() || password.is_empty() || token.is_empty())
        {
            return Err(ZelloError::ConfigError(
                "Username, password, and auth token cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}

/// Zello client for interacting with the Zello API
#[derive(Debug)]
pub struct ZelloClient {
    protocol: Protocol,
    config: ZelloConfig,
    authenticated: bool,
    active_streams: HashMap<u32, StreamInfo>,
    active_inbound_streams: HashMap<u32, StreamInfo>,
    refresh_token: String,
}

/// Attributes of a Zello stream
#[derive(Debug, Clone, Default)]
pub struct StreamInfo {
    pub channel: String,
    pub codec: String,
    pub callsign: Option<String>,
}

/// Zello client for interacting with the Zello API
impl ZelloClient {
    /// Create a new Zello client and connect
    ///
    /// #Errors
    ///
    /// Returns an error if fail to create a new Zello client
    pub async fn new(config: ZelloConfig) -> Result<Self> {
        config.validate()?;

        let protocol = Protocol::connect(None).await?;

        let mut client = Self {
            protocol,
            config,
            authenticated: false,
            active_streams: HashMap::new(),
            active_inbound_streams: HashMap::new(),
            refresh_token: String::new(),
        };

        client.authenticate().await?;

        Ok(client)
    }

    /// Authenticate with the Zello server
    ///
    /// #Errors
    ///
    /// Returns an error if fail to authenticate with the Zello server
    async fn authenticate(&mut self) -> Result<()> {
        let message = match (
            &self.config.username,
            &self.config.password,
            &self.config.auth_token,
        ) {
            (Some(user), Some(password), Some(token)) => Message::logon_password(
                self.protocol.next_seq(),
                user.clone(),
                password.clone(),
                token.clone(),
                self.config.channel.clone(),
            ),
            (_, _, Some(token)) => Message::logon_token(
                self.protocol.next_seq(),
                token.clone(),
                self.config.channel.clone(),
            ),
            _ => {
                return Err(ZelloError::AuthenticationError(
                    "Insufficient Authentication credentials provided".to_string(),
                ));
            }
        };

        self.protocol.send(message).await?;

        // Wait for authentication response
        let response = timeout(Duration::from_secs(10), self.protocol.receive())
            .await
            .map_err(|_| ZelloError::Timeout)?;

        debug!("Received response: {response:?}");

        match response? {
            Some(IncomingMessage::Response(Response::Logon {
                success: true,
                refresh_token,
                ..
            })) => {
                self.authenticated = true;
                self.refresh_token = refresh_token;
                Ok(())
            }

            Some(IncomingMessage::Response(Response::Logon {
                success: false,
                error,
                ..
            })) => Err(ZelloError::AuthenticationError(error.unwrap_or_default())),

            _ => Err(ZelloError::ProtocolError(
                "Unexpected response to logon".to_string(),
            )),
        }
    }

    /// Run the main message processing loop
    ///
    /// # Errors
    ///
    /// Returns an error if message receiving fails
    pub async fn run_message_loop(
        &mut self,
        decoder: Arc<Mutex<Decoder>>,
        pcm_tx: &Sender<Vec<i16>>,
    ) -> Result<()> {
        info!("Listening for messages (press Ctrl+C to exit)...");

        loop {
            match self.receive_message().await {
                Ok(Some(message)) => {
                    handle_message(self, message, decoder.clone(), pcm_tx).await;
                }
                Ok(None) => {
                    info!("Connection closed");
                    break;
                }
                Err(e) => {
                    error!("Error receiving message: {e}");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Send a text message to the channel
    ///
    /// # Errors
    ///
    /// Returns an error if fail to send a text message to the channel
    pub async fn send_text_message(&mut self, text: &str) -> Result<()> {
        if !self.authenticated {
            return Err(ZelloError::NotConnected);
        }

        let message = Message::send_text(
            self.protocol.next_seq(),
            self.config.channel.clone(),
            text.to_string(),
        );

        self.protocol.send(message).await?;

        info!(
            "Sent text message to channel [{}]: {}",
            self.config.channel, text
        );

        Ok(())
    }

    /// Send a text message to the channel
    ///
    /// # Errors
    ///
    /// Returns an error if fail to send a text message to the channel
    pub async fn send_text_message_to_callsign(
        &mut self,
        text: &str,
        callsign: &str,
    ) -> Result<()> {
        if !self.authenticated {
            return Err(ZelloError::NotConnected);
        }

        let message = Message::send_text_for_callsign(
            self.protocol.next_seq(),
            self.config.channel.clone(),
            text.to_string(),
            callsign.to_string(),
        );

        self.protocol.send(message).await?;

        info!("Sent text message to callsign [{}]: {}", callsign, text,);

        Ok(())
    }

    /// Start an audio stream
    ///
    /// # Errors
    ///
    /// Returns an error if fail to start an audio stream
    pub async fn start_audio_stream(&mut self, codec: &str, packet_duration: u32) -> Result<u32> {
        if !self.authenticated {
            return Err(ZelloError::NotConnected);
        }

        let seq = self.protocol.next_seq();
        let message = Message::start_stream(
            seq,
            self.config.channel.clone(),
            codec.to_string(),
            packet_duration,
        );

        self.protocol.send(message).await?;

        // Wait for response
        let response = timeout(Duration::from_secs(5), self.protocol.receive())
            .await
            .map_err(|_| ZelloError::Timeout)?;

        match response? {
            Some(IncomingMessage::Response(Response::Generic { success: true, .. })) => {
                let stream_id = seq; // Use seq as stream_id for now
                self.active_streams.insert(
                    stream_id,
                    StreamInfo {
                        channel: self.config.channel.clone(),
                        codec: codec.to_string(),
                        ..Default::default()
                    },
                );
                Ok(stream_id)
            }
            Some(IncomingMessage::Response(Response::Generic {
                success: false,
                error,
                ..
            })) => Err(ZelloError::AudioError(
                error.unwrap_or_else(|| "Failed to start stream".to_string()),
            )),
            _ => Err(ZelloError::ProtocolError(
                "Unexpected response to start_stream".to_string(),
            )),
        }
    }

    /// Send audio data packet
    ///
    /// # Errors
    ///
    /// Returns an error if fail to send audio data packet
    pub async fn send_audio_packet(&mut self, stream_id: u32, data: Vec<u8>) -> Result<()> {
        if !self.active_streams.contains_key(&stream_id) {
            return Err(ZelloError::AudioError("Invalid stream ID".to_string()));
        }

        self.protocol.send_audio_data(data).await?;
        Ok(())
    }

    /// Stop an audio stream
    ///
    /// # Errors
    ///
    /// Returns an error if fail to stop an audio stream
    pub async fn stop_audio_stream(&mut self, stream_id: u32) -> Result<()> {
        if !self.active_streams.contains_key(&stream_id) {
            return Err(ZelloError::AudioError("Invalid stream ID".to_string()));
        }

        let message = Message::stop_stream(self.protocol.next_seq(), stream_id);
        self.protocol.send(message).await?;

        self.active_streams.remove(&stream_id);
        Ok(())
    }

    /// Receive the next message
    ///
    /// # Errors
    ///
    /// Returns an error if fail to receive the next message
    pub async fn receive_message(&mut self) -> Result<Option<IncomingMessage>> {
        self.protocol.receive().await
    }

    /// Check if client is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.authenticated
    }

    /// Get the current channel
    pub fn channel(&self) -> &str {
        &self.config.channel
    }

    /// Close the connection
    ///
    /// # Errors
    ///
    /// Returns an error if fail to close the connection
    pub async fn close(self) -> Result<()> {
        self.protocol.close().await
    }

    /// Add a new inbound stream to the client
    ///
    /// # Errors
    ///
    /// Returns an error if fail to add a new inbound stream
    pub fn add_inbound_stream(
        &mut self,
        stream_id: u32,
        channel: String,
        codec: String,
        callsign: Option<String>,
    ) -> Result<()> {
        self.active_inbound_streams.insert(
            stream_id,
            StreamInfo {
                channel,
                codec,
                callsign,
            },
        );
        Ok(())
    }

    /// Get an inbound stream from the client
    pub fn get_inbound_stream(&self, stream_id: u32) -> Option<&StreamInfo> {
        self.active_inbound_streams.get(&stream_id)
    }

    /// Remove an inbound stream from the client
    ///
    /// # Errors
    ///
    /// Returns an error if fail to remove an inbound stream
    pub fn remove_inbound_stream(&mut self, stream_id: u32) -> Result<()> {
        self.active_inbound_streams.remove(&stream_id);
        Ok(())
    }
}

/// Command line interface for Zello
#[derive(Debug)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub token: String,
    pub channel: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = ZelloConfig::new(
            "user".to_string(),
            "pass".to_string(),
            "token".to_string(),
            "channel".to_string(),
        );
        assert!(config.validate().is_ok());

        let config = ZelloConfig::new(
            "user".to_string(),
            "pass".to_string(),
            "token".to_string(),
            String::new(),
        );
        assert!(config.validate().is_err());

        let config = ZelloConfig::new(
            String::new(),
            String::new(),
            "token".to_string(),
            "channel".to_string(),
        );
        assert!(config.validate().is_err());
    }
}
