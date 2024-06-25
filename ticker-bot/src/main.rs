use std::{collections::VecDeque, thread, time::Duration};

use anyhow::Error;

use log::{error, info};
use ticker_bot::{
    client::setup_clients, config::load_config, contract::ContractWithData, wallet::setup_wallets,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    info!("Loading up configuration from config.toml...");
    let config = load_config()?;

    info!("Setting up all grpc clients and wallets...");
    let mut clients = setup_clients(&config).await?;
    let mut wallets = setup_wallets(&config, &clients).await?;

    // Move all contract data to a double ended queue to be able to tick them in a round robin fashion
    let mut contracts = VecDeque::new();
    for contract in config.contract {
        contracts.push_back(ContractWithData {
            ctype: contract.ctype,
            address: contract.address,
            chain_prefix: contract.chain_prefix,
            state: vec![],
            balances: vec![],
        });
    }

    loop {
        // Rotate the contract in the Deque
        contracts.rotate_left(1);
        let contract = contracts.back_mut().unwrap();
        let client = clients.get_mut(&contract.chain_prefix).unwrap();
        let wallet = wallets.get_mut(&contract.chain_prefix).unwrap();

        // Let's check if the state or balances of the contract have changed, if so, we tick the contract
        let state = match contract.query_state(client).await {
            Ok(state) => state,
            Err(e) => {
                error!(
                    "Failed to query the state of the contract! Error: {}",
                    e.to_string()
                );
                //TODO: slack alert here
                // Pause a bit as RPC might not be working
                thread::sleep(Duration::from_secs(60));
                continue;
            }
        };

        let balances = match contract.query_balances(client).await {
            Ok(balances) => balances,
            Err(e) => {
                error!(
                    "Failed to query the balances of the contract! Error: {}",
                    e.to_string()
                );
                //TODO: slack alert here
                // Pause a bit as RPC might not be working
                thread::sleep(Duration::from_secs(60));
                continue;
            }
        };

        if state != contract.state || balances != contract.balances {
            info!(
                "State or balances of contract {} have changed, ticking...",
                contract.address
            );
            match contract.tick(wallet).await {
                Ok(_) => info!(
                    "Contract {} has been ticked successfully!",
                    contract.address
                ),
                Err(e) => {
                    error!(
                        "Failed to tick the contract {}! Error: {}",
                        contract.address,
                        e.to_string()
                    );
                    // We will skip sending the slack alert if it's the first iteration (empty state and balances)
                    if !contract.state.is_empty() && !contract.balances.is_empty() {
                        //TODO: slack alert here
                    }
                }
            }

            // Update the state and balances of the contract
            contract.state = state;
            contract.balances = balances;
        } else {
            info!(
                "State and balances of contract {} have not changed, skipping...",
                contract.address
            );
        }

        // Sleep for a bit to avoid spamming the RPC
        thread::sleep(Duration::from_secs(5));
    }
}
