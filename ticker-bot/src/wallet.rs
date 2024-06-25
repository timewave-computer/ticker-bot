use std::{collections::HashMap, str::FromStr};

use anyhow::Error;
use cosmos_grpc_client::{CoinType, Decimal, GrpcClient, Wallet};
use orion::{aead, kdf};

use crate::config::Config;
const MNEMONIC_KEY_LOCATION: &str = "./mnemonic.key";

/// Takes our mnenomic key and the password we introduced to decrypt it and use it for our wallets
pub fn decrypt_mnemonic(mnemonic: &[u8], password: &[u8]) -> Result<Vec<u8>, Error> {
    // Salt I used to generate the secret key when encrypting
    let salt = kdf::Salt::from_slice(&[0u8; 32])?;
    let user_password = kdf::Password::from_slice(password)?;
    let derived_key = kdf::derive_key(&user_password, &salt, 3, 1 << 16, 32)?;
    let decrypted = aead::open(&derived_key, mnemonic)?;
    Ok(decrypted)
}

/// Sets up our wallets for each chain we are going to interact with
pub async fn setup_wallets(
    config: &Config,
    clients: &HashMap<String, GrpcClient>,
) -> Result<HashMap<String, Wallet>, Error> {
    let mut wallets = HashMap::new();
    let mnemonic = std::fs::read(MNEMONIC_KEY_LOCATION)?;
    //let password = rpassword::prompt_password("Introduce your mnemonic key password: ")?;
    let decrypted_mnemonic = decrypt_mnemonic(&mnemonic, b"timewave")?;

    for chain in config.chain.iter() {
        let wallet = Wallet::from_seed_phrase(
            clients.get(&chain.chain_prefix).unwrap().clone(),
            String::from_utf8(decrypted_mnemonic.clone())?,
            &chain.chain_prefix,
            CoinType::Cosmos,
            0,
            Decimal::from_str("0.0025").unwrap(),
            Decimal::from_str("1.5").unwrap(),
            chain.base_denom.clone(),
        )
        .await?;
        wallets.insert(chain.chain_prefix.clone(), wallet);
    }

    Ok(wallets)
}
