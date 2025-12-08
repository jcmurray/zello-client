// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

#![warn(clippy::all, clippy::pedantic, rust_2018_idioms)]
#![warn(missing_debug_implementations, clippy::unwrap_used)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

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
