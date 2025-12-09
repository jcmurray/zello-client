</--
SPDX-License-Identifier: MIT OR Apache-2.0
SPDX-FileCopyrightText: 2024 John C. Murray
-->

# Zello Rust Client

A Rust client library for interacting with the Zello API, supporting WebSocket connections for real-time push-to-talk (PTT) communication.

## Features

- WebSocket-based connection to Zello channels
- Authentication and session management
- Send and receive text messages
- Audio streaming support (send/receive voice messages)
- Async/await support using Tokio
- Type-safe message handling
- Comprehensive error handling

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
zello-client = { path = "." }
tokio = { version = "1.48", features = ["full"] }
```

## Prerequisites

It works on Ubuntu and other Linux distributions with the following audio library dependencies:

- `pulseaudio-utils`,
- `libopus-dev`, and
- `libasound2-dev` packages.

These are the steps to install these required dependencies on distributions that use `apt`:

```bash
sudo apt install pulseaudio-utils
sudo apt install libopus-dev pkg-config
sudo apt install libasound2-dev pkg-config
```

## Quick Start

You need to set up the required credentials for running an application. Example applications are shown below.

### Authentication

Information must be provided to this command in the form of a `.env` file with the following variables:

```bash
ZELLO_USERNAME='your_username'
ZELLO_PASSWORD='your_password'
ZELLO_TOKEN='your Zello authentication token'
ZELLO_CHANNEL='name of a Zello channel'
```

The authentication token is required because this application
is using a development API which requires this token.

This requirement may change in the future when this Zello API is made public.

## Examples

- A simple example showing basic Zello client connection
- A more complete example

Ensure you modify the `examples/.env.example` file with your credentials.

### Simple Example

You can find this example in the `examples` directory of the repository.

```rust,no_run
//! Simple example showing basic Zello client connection

use anyhow::Result;
use zello_client::{
    connect_to_zello, initialize_logging, load_credentials, utilities::load_dotenv_from_file,
};

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv_from_file("examples/.env.example")?;
    initialize_logging()?;
    let credentials = load_credentials()?;
    let mut client = connect_to_zello(&credentials).await?;
    client.send_text_message("Hello from Zello!").await?;
    client.close().await?;
    Ok(())
}
```

### A more complete example

You can find this example in the `examples` directory of the repository.

```rust,no_run
//! Example Zello client application

use anyhow::Result;
use clap::Parser;
use crossbeam_channel::bounded;
use std::sync::Arc;
use tokio::sync::Mutex;
use zello_client::{
    PCM_CHANNEL_CAPACITY, connect_to_zello, create_decoder, initialize_logging, load_credentials,
    setup_audio_output, utilities::load_dotenv_from_file,
};

#[derive(Parser, Debug)]
#[command(name = "zello-client")]
#[command(about = "This is a simple Zello client application that allows you\n\
to listen to audio messages from and to send text messages to other\n\
users on the Zello platform.")]
#[command(
    long_about = "This is a simple Zello client application that allows you\n\
    to listen to audio messages from and to send text messages to other\n\
    users on the Zello platform.

Information must be provided to this command in the form of\n\
a '.env' file with the following variables:

    ZELLO_USERNAME='your_username'
    ZELLO_PASSWORD='your_password'
    ZELLO_TOKEN='your Zello authentication token'
    ZELLO_CHANNEL='name of a Zello channel'

The authentication token is required because this application\n\
is using a development API which requires this token.\n\
\n\
This requirement may change in the future when the API is made public."
)]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(long_version = concat!(env!("CARGO_PKG_VERSION"), " / ", env!("GIT_VERSION")))]
struct Args {
    /// Message to send as text message
    #[arg(short = 'm', long)]
    message: Option<String>,

    /// Destination callsign of message (requires --message)
    #[arg(short = 'c', long, requires = "message")]
    callsign: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    load_dotenv_from_file("examples/.env.example")?;
    initialize_logging()?;

    let credentials = load_credentials()?;
    let decoder = create_decoder()?;

    let (pcm_tx, pcm_rx) = bounded::<Vec<i16>>(PCM_CHANNEL_CAPACITY);
    let pcm_rx = Arc::new(Mutex::new(pcm_rx));
    let _stream = setup_audio_output(pcm_rx)?;

    let mut client = connect_to_zello(&credentials).await?;

    match (args.message, args.callsign) {
        (Some(msg), Some(callsign)) => {
            client
                .send_text_message_to_callsign(&msg, &callsign)
                .await?;
        }
        (Some(msg), None) => {
            client.send_text_message(&msg).await?;
        }
        (None, _) => {
            client.run_message_loop(decoder, &pcm_tx).await?;
        }
    }

    client.close().await?;

    Ok(())
}
```
