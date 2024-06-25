use std::collections::BTreeMap;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

use astroport::asset::{Asset, AssetInfo};
use astroport::factory::{
    InstantiateMsg as FactoryInstantiateMsg, PairConfig, PairType, QueryMsg as FactoryQueryMsg,
};
use astroport::native_coin_registry::ExecuteMsg as NativeCoinRegistryExecuteMsg;
use astroport::native_coin_registry::InstantiateMsg as NativeCoinRegistryInstantiateMsg;
use astroport::pair::ExecuteMsg as PairExecuteMsg;
use astroport::pair::StablePoolParams;

use cosmwasm_std::{coin, Binary, Coin, Decimal, Uint128, Uint64};
use covenant_utils::{InterchainCovenantParty, PoolPriceConfig, SingleSideLpLimits};
use cw_utils::Expiration;
use local_ictest_e2e::utils::constants::{
    ACC_0_ADDRESS_GAIA, ACC_1_ADDRESS_GAIA, ACC_1_ADDRESS_NEUTRON, ADMIN_KEY, BOT_ADDRESS,
    GAIA_CHAIN, LOGS_PATH, NATIVE_ATOM_DENOM, NATIVE_STATOM_DENOM, NEUTRON_CHAIN_ID, STRIDE_CHAIN,
    TICKER_BOT_CONFIG_LOCATION,
};
use local_ictest_e2e::utils::file_system::{read_chains_file, read_logs_file};
use local_ictest_e2e::utils::ibc::{get_ibc_denom, ibc_send};
use local_ictest_e2e::utils::liquid_staking::set_up_host_zone;
use local_ictest_e2e::utils::queries::query_block_height;
use local_ictest_e2e::utils::setup::fund_address;
use local_ictest_e2e::utils::stride::liquid_stake;
use local_ictest_e2e::utils::{
    constants::{ACC_0_ADDRESS_NEUTRON, ACC_0_KEY, API_URL, CHAIN_CONFIG_PATH, NEUTRON_CHAIN},
    setup::{store_astroport_contracts, store_valence_contracts},
    test_context::TestContext,
};

use localic_std::modules::cosmwasm::{contract_execute, contract_query};
use localic_std::{
    errors::LocalError,
    modules::cosmwasm::{contract_instantiate, CosmWasm},
    polling::poll_for_start,
};
use reqwest::blocking::Client;
use ticker_bot::config::{Chain, Config, Contract, ContractType};
use valence_astroport_liquid_pooler::msg::AstroportLiquidPoolerConfig;
use valence_covenant_single_party_pol::msg::{
    CovenantContractCodeIds, CovenantPartyConfig, InstantiateMsg as SinglePartyPolInstantiateMsg,
    LiquidPoolerConfig, LsInfo, QueryMsg as SinglePartyPolQueryMsg, RemoteChainSplitterConfig,
    Timeouts,
};

// Run 'local-ic start neutron_gaia --api-port 42069' before running the tests
fn main() -> Result<(), LocalError> {
    let client = Client::new();
    poll_for_start(&client, API_URL, 300)?;

    let configured_chains = read_chains_file(CHAIN_CONFIG_PATH).unwrap();

    let mut test_ctx = TestContext::from(configured_chains);

    let mut cw = CosmWasm::new(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN),
    );

    // Store all the contracts
    let valence_code_ids = store_valence_contracts(&mut cw)?;
    let astroport_code_ids = store_astroport_contracts(&mut cw)?;

    let code_id_covenant_single_party_pol = valence_code_ids[0];
    let code_id_astroport_liquid_pooler = valence_code_ids[1];
    let code_id_ibc_forwarder = valence_code_ids[2];
    let code_id_interchain_router = valence_code_ids[3];
    let code_id_remote_chain_splitter = valence_code_ids[4];
    let code_id_single_party_pol_holder = valence_code_ids[5];
    let code_id_stride_single_staker = valence_code_ids[6];

    let code_id_astroport_factory = astroport_code_ids[0];
    let code_id_astroport_native_coin_registry = astroport_code_ids[1];
    let code_id_astroport_pair_stable = astroport_code_ids[2];
    let code_id_astroport_token = astroport_code_ids[3];
    let code_id_astroport_whitelist = astroport_code_ids[4];

    // Instantiate astroport and make an stATOM/ATOM stable pair
    let native_coin_registry_instantiate_msg = NativeCoinRegistryInstantiateMsg {
        owner: ACC_0_ADDRESS_NEUTRON.to_string(),
    };

    let native_coin_registry_contract = contract_instantiate(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN),
        ACC_0_KEY,
        code_id_astroport_native_coin_registry,
        &serde_json::to_string(&native_coin_registry_instantiate_msg).unwrap(),
        "native-coin-registry",
        None,
        "",
    )?;

    let factory_instantiate_msg = FactoryInstantiateMsg {
        pair_configs: vec![PairConfig {
            code_id: code_id_astroport_pair_stable,
            pair_type: PairType::Stable {},
            total_fee_bps: 0,
            maker_fee_bps: 0,
            is_disabled: false,
            is_generator_disabled: true,
        }],
        token_code_id: code_id_astroport_token,
        fee_address: None,
        generator_address: None,
        owner: ACC_0_ADDRESS_NEUTRON.to_string(),
        whitelist_code_id: code_id_astroport_whitelist,
        coin_registry_address: native_coin_registry_contract.address.to_string(),
    };

    let factory_contract = contract_instantiate(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN),
        ACC_0_KEY,
        code_id_astroport_factory,
        &serde_json::to_string(&factory_instantiate_msg).unwrap(),
        "astroport-factory",
        None,
        "",
    )?;

    // Add the coins to the registry
    let neutron_statom_denom = get_ibc_denom(
        NATIVE_STATOM_DENOM,
        &test_ctx
            .get_transfer_channels()
            .src(STRIDE_CHAIN)
            .dest(NEUTRON_CHAIN)
            .get(),
    );

    let neutron_atom_denom = get_ibc_denom(
        NATIVE_ATOM_DENOM,
        &test_ctx
            .get_transfer_channels()
            .src(GAIA_CHAIN)
            .dest(NEUTRON_CHAIN)
            .get(),
    );

    let add_to_registry_msg = NativeCoinRegistryExecuteMsg::Add {
        native_coins: vec![
            (neutron_atom_denom.clone(), 6),
            (neutron_statom_denom.clone(), 6),
        ],
    };

    contract_execute(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN),
        &native_coin_registry_contract.address,
        ACC_0_KEY,
        &serde_json::to_string(&add_to_registry_msg).unwrap(),
        "--fees=250000untrn --gas=auto --gas-adjustment=3.0",
    )?;

    // Wait for the coins to be added
    thread::sleep(Duration::from_secs(5));

    // Create the stable pair
    let create_pair_msg = astroport::factory::ExecuteMsg::CreatePair {
        pair_type: PairType::Stable {},
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: neutron_atom_denom.clone(),
            },
            AssetInfo::NativeToken {
                denom: neutron_statom_denom.clone(),
            },
        ],
        init_params: Some(Binary::from(
            serde_json::to_vec(&StablePoolParams {
                amp: 3,
                owner: None,
            })
            .unwrap(),
        )),
    };

    contract_execute(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN),
        &factory_contract.address,
        ACC_0_KEY,
        &serde_json::to_string(&create_pair_msg).unwrap(),
        "--fees=250000untrn --gas=auto --gas-adjustment=3.0",
    )?;

    // Liquid stake some ATOM to get stATOM on Stride to add liquidity to the astroport pool

    // First we have to register Gaia as a host zone and register validators that we can stake on
    set_up_host_zone(&mut test_ctx);

    // Now we can stake using autopilot
    // transfer some atom to stride
    let amount_to_liquid_stake = 50000000;
    ibc_send(
        test_ctx
            .get_request_builder()
            .get_request_builder(GAIA_CHAIN),
        ACC_0_KEY,
        &test_ctx.get_admin_addr().src(STRIDE_CHAIN).get(),
        coin(amount_to_liquid_stake, "uatom"),
        coin(10000, "uatom"),
        &test_ctx
            .get_transfer_channels()
            .src(GAIA_CHAIN)
            .dest(STRIDE_CHAIN)
            .get(),
        None,
    )
    .unwrap();

    // Wait for coins to arrive
    thread::sleep(Duration::from_secs(5));

    // liquid stake the ibc'd atoms for stuatom
    liquid_stake(
        test_ctx
            .get_request_builder()
            .get_request_builder(STRIDE_CHAIN),
        "uatom",
        amount_to_liquid_stake,
    )
    .unwrap();

    // send the stATOM to neutron to eventually add it to the pool
    ibc_send(
        test_ctx
            .get_request_builder()
            .get_request_builder(STRIDE_CHAIN),
        ADMIN_KEY,
        ACC_0_ADDRESS_NEUTRON,
        coin(amount_to_liquid_stake, NATIVE_STATOM_DENOM),
        coin(100000, "ustrd"),
        &test_ctx
            .get_transfer_channels()
            .src(STRIDE_CHAIN)
            .dest(NEUTRON_CHAIN)
            .get(),
        None,
    )?;

    // Send some ATOM as well
    ibc_send(
        test_ctx
            .get_request_builder()
            .get_request_builder(GAIA_CHAIN),
        ACC_0_ADDRESS_GAIA,
        ACC_0_ADDRESS_NEUTRON,
        coin(50000000, NATIVE_ATOM_DENOM),
        coin(100000, "uatom"),
        &test_ctx
            .get_transfer_channels()
            .src(GAIA_CHAIN)
            .dest(NEUTRON_CHAIN)
            .get(),
        None,
    )?;

    // Get the pool address
    let pair_info = contract_query(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN),
        &factory_contract.address,
        &serde_json::to_string(&FactoryQueryMsg::Pair {
            asset_infos: vec![
                AssetInfo::NativeToken {
                    denom: neutron_atom_denom.clone(),
                },
                AssetInfo::NativeToken {
                    denom: neutron_statom_denom.clone(),
                },
            ],
        })
        .unwrap(),
    );

    let pool_addr = pair_info["data"]["contract_addr"].as_str().unwrap();
    //let lp_token_addr = pair_info["data"]["liquidity_token"].as_str().unwrap();

    // Provide liquidity (10 ATOM / 10 stATOM)
    let provide_liquidity_msg = PairExecuteMsg::ProvideLiquidity {
        assets: vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: neutron_atom_denom.clone(),
                },
                amount: Uint128::from(10000000u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: neutron_statom_denom.clone(),
                },
                amount: Uint128::from(10000000u128),
            },
        ],
        slippage_tolerance: Some(Decimal::percent(1)),
        auto_stake: Some(false),
        receiver: Some(ACC_0_ADDRESS_NEUTRON.to_string()),
    };

    contract_execute(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN),
        pool_addr,
        ACC_0_KEY,
        &serde_json::to_string(&provide_liquidity_msg).unwrap(),
        &format!("--amount 10000000{neutron_atom_denom},10000000{neutron_statom_denom} --fees=250000untrn --gas=auto --gas-adjustment=3.0"),
    )?;

    thread::sleep(Duration::from_secs(5));

    // Now we can start the covenants
    let current_height = query_block_height(NEUTRON_CHAIN_ID.to_string());
    let instantiate_covenant_msg = SinglePartyPolInstantiateMsg {
        label: "single_party_pol_covenant".to_string(),
        timeouts: Timeouts {
            ica_timeout: Uint64::new(10000),
            ibc_transfer_timeout: Uint64::new(10000),
        },
        contract_codes: CovenantContractCodeIds {
            ibc_forwarder_code: code_id_ibc_forwarder,
            holder_code: code_id_single_party_pol_holder,
            remote_chain_splitter_code: code_id_remote_chain_splitter,
            liquid_pooler_code: code_id_astroport_liquid_pooler,
            liquid_staker_code: code_id_stride_single_staker,
            interchain_router_code: code_id_interchain_router,
        },
        clock_tick_max_gas: None,
        lockup_period: Expiration::AtHeight(current_height + 110),
        ls_info: LsInfo {
            ls_denom: NATIVE_STATOM_DENOM.to_string(),
            ls_denom_on_neutron: neutron_statom_denom.to_string(),
            ls_chain_to_neutron_channel_id: test_ctx
                .get_transfer_channels()
                .src(STRIDE_CHAIN)
                .dest(NEUTRON_CHAIN)
                .get(),
            ls_neutron_connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN)
                .dest(STRIDE_CHAIN)
                .get(),
        },
        ls_forwarder_config: CovenantPartyConfig::Interchain(InterchainCovenantParty {
            party_receiver_addr: ACC_0_ADDRESS_NEUTRON.to_string(),
            party_chain_connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN)
                .dest(GAIA_CHAIN)
                .get(),
            ibc_transfer_timeout: Uint64::new(10000),
            party_to_host_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(GAIA_CHAIN)
                .dest(STRIDE_CHAIN)
                .get(),
            host_to_party_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(STRIDE_CHAIN)
                .dest(GAIA_CHAIN)
                .get(),
            remote_chain_denom: NATIVE_ATOM_DENOM.to_string(),
            addr: ACC_0_ADDRESS_NEUTRON.to_string(),
            native_denom: get_ibc_denom(
                NATIVE_ATOM_DENOM,
                &test_ctx
                    .get_transfer_channels()
                    .src(GAIA_CHAIN)
                    .dest(NEUTRON_CHAIN)
                    .get(),
            ),
            contribution: Coin {
                denom: NATIVE_ATOM_DENOM.to_string(),
                amount: Uint128::new(10000000),
            },
            denom_to_pfm_map: BTreeMap::new(),
            fallback_address: None,
        }),
        lp_forwarder_config: CovenantPartyConfig::Interchain(InterchainCovenantParty {
            party_receiver_addr: ACC_0_ADDRESS_NEUTRON.to_string(),
            party_chain_connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN)
                .dest(GAIA_CHAIN)
                .get(),
            ibc_transfer_timeout: Uint64::new(10000),
            party_to_host_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(GAIA_CHAIN)
                .dest(NEUTRON_CHAIN)
                .get(),
            host_to_party_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(NEUTRON_CHAIN)
                .dest(GAIA_CHAIN)
                .get(),
            remote_chain_denom: NATIVE_ATOM_DENOM.to_string(),
            addr: ACC_0_ADDRESS_NEUTRON.to_string(),
            native_denom: get_ibc_denom(
                NATIVE_ATOM_DENOM,
                &test_ctx
                    .get_transfer_channels()
                    .src(GAIA_CHAIN)
                    .dest(NEUTRON_CHAIN)
                    .get(),
            ),
            contribution: Coin {
                denom: NATIVE_ATOM_DENOM.to_string(),
                amount: Uint128::new(10000000),
            },
            denom_to_pfm_map: BTreeMap::new(),
            fallback_address: None,
        }),
        pool_price_config: PoolPriceConfig {
            expected_spot_price: Decimal::one(),
            acceptable_price_spread: Decimal::from_str("0.1").unwrap(),
        },
        remote_chain_splitter_config: RemoteChainSplitterConfig {
            channel_id: test_ctx
                .get_transfer_channels()
                .src(NEUTRON_CHAIN)
                .dest(GAIA_CHAIN)
                .get(),
            connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN)
                .dest(GAIA_CHAIN)
                .get(),
            denom: NATIVE_ATOM_DENOM.to_string(),
            amount: Uint128::from(10000000u128),
            ls_share: Decimal::percent(50),
            native_share: Decimal::percent(50),
            fallback_address: None,
        },
        emergency_committee: None,
        covenant_party_config: InterchainCovenantParty {
            party_receiver_addr: ACC_1_ADDRESS_GAIA.to_string(),
            party_chain_connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN)
                .dest(GAIA_CHAIN)
                .get(),
            ibc_transfer_timeout: Uint64::new(300),
            party_to_host_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(GAIA_CHAIN)
                .dest(NEUTRON_CHAIN)
                .get(),
            host_to_party_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(NEUTRON_CHAIN)
                .dest(GAIA_CHAIN)
                .get(),
            remote_chain_denom: NATIVE_ATOM_DENOM.to_string(),
            addr: ACC_1_ADDRESS_NEUTRON.to_string(),
            native_denom: get_ibc_denom(
                NATIVE_ATOM_DENOM,
                &test_ctx
                    .get_transfer_channels()
                    .src(GAIA_CHAIN)
                    .dest(NEUTRON_CHAIN)
                    .get(),
            ),
            contribution: Coin {
                denom: NATIVE_ATOM_DENOM.to_string(),
                amount: Uint128::new(20000000),
            },
            denom_to_pfm_map: BTreeMap::new(),
            fallback_address: None,
        },
        liquid_pooler_config: LiquidPoolerConfig::Astroport(AstroportLiquidPoolerConfig {
            pool_pair_type: PairType::Stable {},
            pool_address: pool_addr.to_string(),
            asset_a_denom: neutron_atom_denom.clone(),
            asset_b_denom: neutron_statom_denom.clone(),
            single_side_lp_limits: SingleSideLpLimits {
                asset_a_limit: Uint128::new(1000000),
                asset_b_limit: Uint128::new(1000000),
            },
        }),
    };

    let contract = contract_instantiate(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN),
        ACC_0_KEY,
        code_id_covenant_single_party_pol,
        &serde_json::to_string(&instantiate_covenant_msg).unwrap(),
        "single-party-pol-covenant",
        None,
        "",
    )?;

    // Now that we successfully instantiated the covenant, let's get all the addresses that we will add to our ticker bot so it can start ticking them
    let ntrn_request_builder = test_ctx
        .get_request_builder()
        .get_request_builder(NEUTRON_CHAIN);

    let query_response = contract_query(
        ntrn_request_builder,
        &contract.address,
        &serde_json::to_string(&SinglePartyPolQueryMsg::LiquidStakerAddress {}).unwrap(),
    );
    let liquid_staker_address = query_response["data"].as_str().unwrap();

    let query_response = contract_query(
        ntrn_request_builder,
        &contract.address,
        &serde_json::to_string(&SinglePartyPolQueryMsg::IbcForwarderAddress {
            ty: "ls".to_string(),
        })
        .unwrap(),
    );
    let ibc_forwarder_address = query_response["data"].as_str().unwrap();

    let query_response = contract_query(
        ntrn_request_builder,
        &contract.address,
        &serde_json::to_string(&SinglePartyPolQueryMsg::IbcForwarderAddress {
            ty: "lp".to_string(),
        })
        .unwrap(),
    );
    let liquid_pooler_forwarder_address = query_response["data"].as_str().unwrap();

    let query_response = contract_query(
        ntrn_request_builder,
        &contract.address,
        &serde_json::to_string(&SinglePartyPolQueryMsg::LiquidPoolerAddress {}).unwrap(),
    );
    let liquid_pooler_address = query_response["data"].as_str().unwrap();

    let query_response = contract_query(
        ntrn_request_builder,
        &contract.address,
        &serde_json::to_string(&SinglePartyPolQueryMsg::SplitterAddress {}).unwrap(),
    );
    let remote_chain_splitter_address = query_response["data"].as_str().unwrap();

    let query_response = contract_query(
        ntrn_request_builder,
        &contract.address,
        &serde_json::to_string(&SinglePartyPolQueryMsg::InterchainRouterAddress {}).unwrap(),
    );
    let interchain_router_address = query_response["data"].as_str().unwrap();

    let query_response = contract_query(
        ntrn_request_builder,
        &contract.address,
        &serde_json::to_string(&SinglePartyPolQueryMsg::HolderAddress {}).unwrap(),
    );
    let holder_address = query_response["data"].as_str().unwrap();

    // Get GRPC endpoint for bot
    let logs = read_logs_file(LOGS_PATH).unwrap();
    let mut grpc_endpoint = String::default();
    for chain in logs.chains {
        if chain.chain_id == NEUTRON_CHAIN_ID {
            grpc_endpoint = chain.grpc_address;
            break;
        }
    }

    let ticker_bot_config = Config {
        contract: vec![
            Contract {
                ctype: ContractType::StrideLiquidStaker,
                chain_prefix: "neutron".to_string(),
                address: liquid_staker_address.to_string(),
            },
            Contract {
                ctype: ContractType::IbcForwarder,
                chain_prefix: "neutron".to_string(),
                address: ibc_forwarder_address.to_string(),
            },
            Contract {
                ctype: ContractType::IbcForwarder,
                chain_prefix: "neutron".to_string(),
                address: liquid_pooler_forwarder_address.to_string(),
            },
            Contract {
                ctype: ContractType::AstroportLiquidPooler,
                chain_prefix: "neutron".to_string(),
                address: liquid_pooler_address.to_string(),
            },
            Contract {
                ctype: ContractType::RemoteChainSplitter,
                chain_prefix: "neutron".to_string(),
                address: remote_chain_splitter_address.to_string(),
            },
            Contract {
                ctype: ContractType::InterchainRouter,
                chain_prefix: "neutron".to_string(),
                address: interchain_router_address.to_string(),
            },
        ],
        chain: vec![Chain {
            chain_prefix: "neutron".to_string(),
            base_denom: "untrn".to_string(),
            endpoint: format!("https://{grpc_endpoint}"),
        }],
    };

    // Overwrite the ticker-bot config
    let config = toml::to_string(&ticker_bot_config).unwrap();
    std::fs::write(TICKER_BOT_CONFIG_LOCATION, config).unwrap();

    // Fund all addresses with NTRN
    fund_address(
        ntrn_request_builder,
        liquid_staker_address,
        Uint128::new(10000000),
    )?;
    fund_address(
        ntrn_request_builder,
        ibc_forwarder_address,
        Uint128::new(10000000),
    )?;
    fund_address(
        ntrn_request_builder,
        liquid_pooler_forwarder_address,
        Uint128::new(10000000),
    )?;
    fund_address(
        ntrn_request_builder,
        liquid_pooler_address,
        Uint128::new(10000000),
    )?;
    fund_address(
        ntrn_request_builder,
        remote_chain_splitter_address,
        Uint128::new(10000000),
    )?;
    fund_address(
        ntrn_request_builder,
        interchain_router_address,
        Uint128::new(10000000),
    )?;
    fund_address(ntrn_request_builder, holder_address, Uint128::new(10000000))?;

    // Fund the bot as well so that it has enough for fees
    fund_address(ntrn_request_builder, BOT_ADDRESS, Uint128::new(10000000))?;

    println!("Bot is now ready to run!");

    Ok(())
}
