// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

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
