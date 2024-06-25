use std::path::Path;

use cosmwasm_std::{Coin, Uint128};
use localic_std::{
    errors::LocalError,
    modules::{bank::send, cosmwasm::CosmWasm},
    transactions::ChainRequestBuilder,
};

use super::constants::ACC_0_KEY;

pub fn store_valence_contracts(cw: &mut CosmWasm) -> Result<Vec<u64>, LocalError> {
    let abs_path_wasms: std::path::PathBuf =
        std::fs::canonicalize(Path::new("./local-interchaintest/wasms/valence")).unwrap();

    let code_id_covenant_single_party_pol = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/valence_covenant_single_party_pol.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_astroport_liquid_pooler = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/valence_astroport_liquid_pooler.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_ibc_forwarder = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/valence_ibc_forwarder.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_interchain_router = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/valence_interchain_router.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_remote_chain_splitter = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/valence_remote_chain_splitter.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_single_party_pol_holder = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/valence_single_party_pol_holder.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_stride_liquid_staker = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/valence_stride_liquid_staker.wasm",
            abs_path_wasms.display()
        )),
    )?;

    Ok(vec![
        code_id_covenant_single_party_pol,
        code_id_astroport_liquid_pooler,
        code_id_ibc_forwarder,
        code_id_interchain_router,
        code_id_remote_chain_splitter,
        code_id_single_party_pol_holder,
        code_id_stride_liquid_staker,
    ])
}

pub fn store_astroport_contracts(cw: &mut CosmWasm) -> Result<Vec<u64>, LocalError> {
    let abs_path_wasms: std::path::PathBuf =
        std::fs::canonicalize(Path::new("./local-interchaintest/wasms/astroport")).unwrap();

    let code_id_astroport_factory = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/astroport_factory.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_astroport_native_coin_registry = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/astroport_native_coin_registry.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_astroport_pair_stable = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/astroport_pair_stable.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_astroport_token = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/astroport_token.wasm",
            abs_path_wasms.display()
        )),
    )?;

    let code_id_astroport_whitelist = cw.store(
        ACC_0_KEY,
        Path::new(&format!(
            "{}/astroport_whitelist.wasm",
            abs_path_wasms.display()
        )),
    )?;

    Ok(vec![
        code_id_astroport_factory,
        code_id_astroport_native_coin_registry,
        code_id_astroport_pair_stable,
        code_id_astroport_token,
        code_id_astroport_whitelist,
    ])
}

pub fn fund_address(
    rb: &ChainRequestBuilder,
    address: &str,
    amount: Uint128,
) -> Result<(), LocalError> {
    send(
        rb,
        ACC_0_KEY,
        address,
        &[Coin {
            denom: "untrn".to_string(),
            amount,
        }],
        &Coin {
            denom: "untrn".to_string(),
            amount: Uint128::new(5000),
        },
    )?;
    Ok(())
}
