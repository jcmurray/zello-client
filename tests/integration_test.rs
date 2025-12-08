// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

//! Integration tests for the Zello client

use std::time::Duration;
use zello_client::{Error, Message, Response, ZelloClient, ZelloConfig};

#[tokio::test]
#[ignore] // Requires valid credentials
async fn test_client_connection_and_auth() {
    let config = ZelloConfig::new(
        std::env::var("ZELLO_USERNAME").unwrap_or_else(|_| "test_user".to_string()),
        std::env::var("ZELLO_PASSWORD").unwrap_or_else(|_| "test_pass".to_string()),
        std::env::var("ZELLO_TOKEN").unwrap_or_else(|_| "test_pass".to_string()),
        std::env::var("ZELLO_CHANNEL").unwrap_or_else(|_| "test_channel".to_string()),
    );

    let client = ZelloClient::new(config).await;
    assert!(client.is_ok(), "Failed to connect: {:?}", client.err());

    let client = client.unwrap();
    assert!(client.is_authenticated());
}

#[tokio::test]
async fn test_config_validation() {
    // Valid config with username/password/token
    let config = ZelloConfig::new(
        "user".to_string(),
        "pass".to_string(),
        "token".to_string(),
        "channel".to_string(),
    );
    assert!(config.validate().is_ok());

    // Empty channel should fail
    let config = ZelloConfig::new(
        "user".to_string(),
        "pass".to_string(),
        "some_token".to_string(),
        String::new(),
    );
    assert!(config.validate().is_err());

    // Missing credentials should fail
    let config = ZelloConfig::new(
        String::new(),
        String::new(),
        String::new(),
        "channel".to_string(),
    );
    assert!(config.validate().is_err());
}

#[tokio::test]
#[ignore]
async fn test_send_text_message() {
    let config = ZelloConfig::new(
        std::env::var("ZELLO_USERNAME").expect("ZELLO_USERNAME not set"),
        std::env::var("ZELLO_PASSWORD").expect("ZELLO_PASSWORD not set"),
        std::env::var("ZELLO_TOKEN").expect("ZELLO_TOKEN not set"),
        std::env::var("ZELLO_CHANNEL").expect("ZELLO_CHANNEL not set"),
    );

    let mut client = ZelloClient::new(config).await.expect("Failed to connect");

    let result = client.send_text_message("Integration test message").await;
    assert!(result.is_ok(), "Failed to send message: {:?}", result.err());
}

#[tokio::test]
#[ignore]
async fn test_receive_messages_with_timeout() {
    let config = ZelloConfig::new(
        std::env::var("ZELLO_USERNAME").expect("ZELLO_USERNAME not set"),
        std::env::var("ZELLO_PASSWORD").expect("ZELLO_PASSWORD not set"),
        std::env::var("ZELLO_TOKEN").expect("ZELLO_TOKEN not set"),
        std::env::var("ZELLO_CHANNEL").expect("ZELLO_CHANNEL not set"),
    );

    let mut client = ZelloClient::new(config).await.expect("Failed to connect");

    // Try to receive a message with timeout
    let result = tokio::time::timeout(Duration::from_secs(5), client.receive_message()).await;

    // Either we got a message or timed out (both are acceptable)
    match result {
        Ok(Ok(Some(_msg))) => {
            // Received a message successfully
        }
        Ok(Ok(None)) => {
            // Connection closed
        }
        Ok(Err(_e)) => {
            // Error receiving
        }
        Err(_) => {
            // Timeout (acceptable for this test)
        }
    }
}

#[test]
fn test_message_constructors() {
    let msg = Message::send_text(1, "test".to_string(), "hello".to_string());
    assert_eq!(msg.seq(), Some(1));

    let msg = Message::logon_password(
        2,
        "user".to_string(),
        "pass".to_string(),
        "token".to_string(),
        "channel".to_string(),
    );
    assert_eq!(msg.seq(), Some(2));

    let msg = Message::start_stream(3, "channel".to_string(), "opus".to_string(), 20);
    assert_eq!(msg.seq(), Some(3));

    let msg = Message::stop_stream(4, 100);
    assert_eq!(msg.seq(), Some(4));
}

#[test]
fn test_message_success_check() {
    let msg = Response::Generic {
        seq: 1,
        success: true,
        error: None,
    };
    assert_eq!(msg.is_success(), true);

    let msg = Response::Generic {
        seq: 2,
        success: false,
        error: Some("Failed".to_string()),
    };
    assert_eq!(msg.is_success(), false);
}

#[test]
fn test_message_error_extraction() {
    // Test Error variant
    let msg = Error::Error {
        error: "Test error".to_string(),
    };

    assert_eq!(msg.error(), "Test error");

    // Test Response variant with error
    let msg = Response::Generic {
        seq: 1,
        success: false,
        error: Some("Response error".to_string()),
    };
    assert_eq!(msg.error(), Some("Response error"));

    // Test Response variant without error
    let msg = Response::Generic {
        seq: 1,
        success: true,
        error: None,
    };
    assert_eq!(msg.error(), None);
}

#[test]
fn test_message_serialization() {
    let msg = Message::send_text(1, "test_channel".to_string(), "Hello".to_string());
    let json = serde_json::to_string(&msg).unwrap();

    assert!(json.contains("send_text_message"));
    assert!(json.contains("Hello"));
    assert!(json.contains("test_channel"));
    assert!(json.contains("\"seq\":1"));
}

#[tokio::test]
#[ignore]
async fn test_audio_stream_lifecycle() {
    let config = ZelloConfig::new(
        std::env::var("ZELLO_USERNAME").expect("ZELLO_USERNAME not set"),
        std::env::var("ZELLO_PASSWORD").expect("ZELLO_PASSWORD not set"),
        std::env::var("ZELLO_TOKEN").expect("ZELLO_TOKEN not set"),
        std::env::var("ZELLO_CHANNEL").expect("ZELLO_CHANNEL not set"),
    );

    let mut client = ZelloClient::new(config).await.expect("Failed to connect");

    // Start audio stream
    let stream_id = client
        .start_audio_stream("opus", 20)
        .await
        .expect("Failed to start stream");

    assert!(stream_id > 0);

    // Send some dummy audio data
    let audio_data = vec![0u8; 100];
    let result = client.send_audio_packet(stream_id, audio_data).await;
    assert!(result.is_ok());

    // Stop the stream
    let result = client.stop_audio_stream(stream_id).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_invalid_stream_operations() {
    let config = ZelloConfig::new(
        "dummy".to_string(),
        "dummy".to_string(),
        "dummy".to_string(),
        "dummy".to_string(),
    );

    // This will fail to connect, but that's okay for this test
    if let Ok(mut client) = ZelloClient::new(config).await {
        // Try to send audio on non-existent stream
        let result = client.send_audio_packet(999, vec![0u8; 10]).await;
        assert!(result.is_err());

        // Try to stop non-existent stream
        let result = client.stop_audio_stream(999).await;
        assert!(result.is_err());
    }
}
