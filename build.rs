// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2024 John C. Murray

#![warn(clippy::all, clippy::pedantic, rust_2018_idioms)]
#![warn(missing_debug_implementations, clippy::unwrap_used)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::{fs, io, process::Command};

const GIT_FILE_NAME: &str = "src/git_version.txt";
const GIT_COMMAND: &str = "git";
const GIT_ARGUMENTS: &[&str] = &["describe", "--tags", "--dirty", "--always", "--long"];
const UNKNOWN_VERSION: &str = "unknown";

fn main() {
    let git_version = get_git_version().unwrap_or_else(|e| {
        eprintln!("cargo:warning=Failed to get git version: {e}");
        UNKNOWN_VERSION.to_string()
    });

    println!("cargo:rustc-env=GIT_VERSION={git_version}");
}

fn get_git_version() -> io::Result<String> {
    let output = Command::new(GIT_COMMAND).args(GIT_ARGUMENTS).output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(io::Error::other(format!(
            "git command failed with status: {}",
            output.status
        )))
    }
}
