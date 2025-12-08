// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

//! Zello protocol implementation

use bytes::Buf;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tracing::debug;
use tungstenite::protocol::Message as WsMessage;

use crate::ZELLO_DEFAULT_URL;
use crate::error::{Result, ZelloError};
use crate::message::{Event, IncomingMessage, Message};

/// Zello protocol handler
#[derive(Debug)]
pub struct Protocol {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    sequence: u32,
}

impl Protocol {
    /// Connect to Zello server
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket connection fails
    pub async fn connect(url: Option<&str>) -> Result<Self> {
        let url = url.unwrap_or(ZELLO_DEFAULT_URL);
        let (ws, _) = connect_async(url)
            .await
            .map_err(|e| ZelloError::ConnectionError(e.to_string()))?;

        Ok(Self { ws, sequence: 1 })
    }

    /// Send a message
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or sending fails
    pub async fn send(&mut self, message: Message) -> Result<()> {
        let json = serde_json::to_string(&message)?;
        debug!("Sending message: {json}");
        self.ws
            .send(WsMessage::Text(json.into()))
            .await
            .map_err(|e| ZelloError::ConnectionError(e.to_string()))?;
        Ok(())
    }

    /// Send a message and return its sequence number
    ///
    /// # Errors
    ///
    /// Returns an error if sending fails
    pub async fn send_with_seq(&mut self, mut message: Message) -> Result<u32> {
        let seq = self.next_seq();

        // Set sequence number based on message type
        match &mut message {
            Message::Logon { seq: s, .. }
            | Message::SendTextMessage { seq: s, .. }
            | Message::StartStream { seq: s, .. }
            | Message::StopStream { seq: s, .. } => *s = seq,
        }

        self.send(message).await?;
        Ok(seq)
    }

    /// Receive the next message
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or message parsing fails
    pub async fn receive(&mut self) -> Result<Option<IncomingMessage>> {
        loop {
            match self.ws.next().await {
                Some(Ok(WsMessage::Text(text))) => {
                    debug!("Receiving message: {text}");
                    let message: IncomingMessage = serde_json::from_str(&text)?;
                    debug!("Parsed message: {message:?}");
                    return Ok(Some(message));
                }
                Some(Ok(WsMessage::Binary(mut data))) => {
                    let data_length = data.len();
                    let data_type = data.get_u8();
                    let stream_id = data.get_u32();
                    let packet_id = data.get_u32();
                    let audio_data = data.split_to(data.len());

                    debug!(
                        "Received binary message of {data_length} bytes, type: {data_type}, \
                         stream_id: {stream_id}, packet_id: {packet_id}, audio_data_len: {}",
                        audio_data.len()
                    );

                    let message = IncomingMessage::Event(Event::AudioData {
                        stream_id,
                        packet_id,
                        data: audio_data.to_vec(),
                    });
                    return Ok(Some(message));
                }
                Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_))) => {
                    // Continue loop for ping/pong messages
                }
                Some(Ok(WsMessage::Close(_))) => {
                    return Err(ZelloError::ConnectionError("Connection closed".to_string()));
                }
                Some(Ok(WsMessage::Frame(_))) => {
                    return Err(ZelloError::ProtocolError(
                        "Unexpected frame message".to_string(),
                    ));
                }
                Some(Err(e)) => {
                    return Err(ZelloError::WebSocketError(Box::new(e)));
                }
                None => return Ok(None),
            }
        }
    }

    /// Get the next sequence number
    #[must_use]
    pub fn next_seq(&mut self) -> u32 {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        seq
    }

    /// Close the connection
    ///
    /// # Errors
    ///
    /// Returns an error if closing the WebSocket fails
    pub async fn close(mut self) -> Result<()> {
        self.ws
            .close(None)
            .await
            .map_err(|e| ZelloError::ConnectionError(e.to_string()))?;
        Ok(())
    }

    /// Send raw audio data
    ///
    /// # Errors
    ///
    /// Returns an error if sending fails
    pub async fn send_audio_data(&mut self, data: Vec<u8>) -> Result<()> {
        self.ws
            .send(WsMessage::Binary(data.into()))
            .await
            .map_err(|e| ZelloError::AudioError(e.to_string()))?;
        Ok(())
    }
}
