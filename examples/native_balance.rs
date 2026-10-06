// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Native-balance read using explicit inputs and the public library API.

#[path = "support/inputs.rs"]
mod inputs;

use std::{io, io::Write, process::ExitCode};

use regit_web3::{chains::evm::EvmClient, error::Error};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Error> {
    let inputs = inputs::from_env()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::Configuration)?;
    let observation = runtime.block_on(async {
        let client = EvmClient::connect(inputs.config).await?;
        client.get_native_balance(inputs.address, None).await
    })?;
    let json = serde_json::to_string(&observation).map_err(|_| Error::Configuration)?;
    writeln!(io::stdout().lock(), "{json}").map_err(|_| Error::Configuration)
}
