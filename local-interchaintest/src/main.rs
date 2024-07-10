use std::collections::BTreeMap;
use std::error::Error;
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

use cosmwasm_std::{Binary, Coin, Decimal, Uint128, Uint64};
use covenant_utils::{InterchainCovenantParty, PoolPriceConfig, SingleSideLpLimits};
use cw_utils::Expiration;
use local_ictest_e2e::utils::constants::{
    ACC_1_ADDRESS_GAIA, ACC_1_ADDRESS_NEUTRON, ADMIN_KEY, ASTROPORT_PATH, BOT_ADDRESS,
    EXECUTE_FLAGS, LOCAL_CODE_ID_CACHE_PATH, LOGS_PATH, NATIVE_STATOM_DENOM,
    TICKER_BOT_CONFIG_LOCATION, VALENCE_PATH,
};

use local_ictest_e2e::utils::file_system::read_logs_file;
use localic_std::modules::bank::send;
use localic_std::modules::cosmwasm::{contract_execute, contract_query};
use localic_std::{modules::cosmwasm::contract_instantiate, polling::poll_for_start};
use localic_utils::{
    ConfigChainBuilder, TestContextBuilder, DEFAULT_KEY, GAIA_CHAIN_NAME, LOCAL_IC_API_URL,
    NEUTRON_CHAIN_ADMIN_ADDR, NEUTRON_CHAIN_ID, NEUTRON_CHAIN_NAME, STRIDE_CHAIN_ADMIN_ADDR,
    STRIDE_CHAIN_NAME,
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
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let client = Client::new();
    poll_for_start(&client, LOCAL_IC_API_URL, 300)?;

    let mut test_ctx = TestContextBuilder::default()
        .with_unwrap_raw_logs(true)
        .with_chain(ConfigChainBuilder::default_neutron().build()?)
        .with_chain(ConfigChainBuilder::default_stride().build()?)
        .with_chain(ConfigChainBuilder::default_gaia().build()?)
        .with_artifacts_dir("artifacts")
        .with_transfer_channels(NEUTRON_CHAIN_NAME, GAIA_CHAIN_NAME)
        .with_transfer_channels(STRIDE_CHAIN_NAME, GAIA_CHAIN_NAME)
        .with_transfer_channels(STRIDE_CHAIN_NAME, NEUTRON_CHAIN_NAME)
        .build()?;

    let mut uploader = test_ctx.build_tx_upload_contracts();
    uploader
        .send_with_local_cache(VALENCE_PATH, NEUTRON_CHAIN_NAME, LOCAL_CODE_ID_CACHE_PATH)
        .unwrap();

    uploader
        .send_with_local_cache(ASTROPORT_PATH, NEUTRON_CHAIN_NAME, LOCAL_CODE_ID_CACHE_PATH)
        .unwrap();

    let astroport_native_coin_registry_code_id = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("astroport_native_coin_registry")
        .unwrap();

    let astroport_pair_stable_code_id = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("astroport_pair_stable")
        .unwrap();

    let astroport_token_code_id = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("astroport_token")
        .unwrap();

    let astroport_whitelist_code_id = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("astroport_whitelist")
        .unwrap();

    let astroport_factory_code_id = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("astroport_factory")
        .unwrap();

    let native_coin_registry_instantiate_msg = NativeCoinRegistryInstantiateMsg {
        owner: NEUTRON_CHAIN_ADMIN_ADDR.to_string(),
    };

    let native_coin_registry_contract = contract_instantiate(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN_NAME),
        DEFAULT_KEY,
        astroport_native_coin_registry_code_id,
        &serde_json::to_string(&native_coin_registry_instantiate_msg).unwrap(),
        "native-coin-registry",
        None,
        "",
    )?;

    let factory_instantiate_msg = FactoryInstantiateMsg {
        pair_configs: vec![PairConfig {
            code_id: astroport_pair_stable_code_id,
            pair_type: PairType::Stable {},
            total_fee_bps: 0,
            maker_fee_bps: 0,
            is_disabled: false,
            is_generator_disabled: true,
        }],
        token_code_id: astroport_token_code_id,
        fee_address: None,
        generator_address: None,
        owner: NEUTRON_CHAIN_ADMIN_ADDR.to_string(),
        whitelist_code_id: astroport_whitelist_code_id,
        coin_registry_address: native_coin_registry_contract.address.to_string(),
    };

    let factory_contract = contract_instantiate(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN_NAME),
        DEFAULT_KEY,
        astroport_factory_code_id,
        &serde_json::to_string(&factory_instantiate_msg).unwrap(),
        "astroport-factory",
        None,
        "",
    )?;

    let atom_denom = test_ctx.get_native_denom().src(GAIA_CHAIN_NAME).get();
    // Add the coins to the registry
    let neutron_statom_denom =
        test_ctx.get_ibc_denom(NATIVE_STATOM_DENOM, STRIDE_CHAIN_NAME, NEUTRON_CHAIN_NAME);

    let neutron_atom_denom =
        test_ctx.get_ibc_denom(&atom_denom, GAIA_CHAIN_NAME, NEUTRON_CHAIN_NAME);

    let add_to_registry_msg = NativeCoinRegistryExecuteMsg::Add {
        native_coins: vec![
            (neutron_atom_denom.clone(), 6),
            (neutron_statom_denom.clone(), 6),
        ],
    };

    contract_execute(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN_NAME),
        &native_coin_registry_contract.address,
        DEFAULT_KEY,
        &serde_json::to_string(&add_to_registry_msg).unwrap(),
        EXECUTE_FLAGS,
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
            .get_request_builder(NEUTRON_CHAIN_NAME),
        &factory_contract.address,
        DEFAULT_KEY,
        &serde_json::to_string(&create_pair_msg).unwrap(),
        EXECUTE_FLAGS,
    )?;

    // Liquid stake some ATOM to get stATOM on Stride to add liquidity to the astroport pool

    // First we have to register Gaia as a host zone
    test_ctx.set_up_stride_host_zone(GAIA_CHAIN_NAME);

    // Now we can liquid stake
    // transfer some atom to stride
    let amount_to_liquid_stake = 50000000;

    test_ctx
        .build_tx_transfer()
        .with_chain_name(GAIA_CHAIN_NAME)
        .with_amount(amount_to_liquid_stake)
        .with_recipient(STRIDE_CHAIN_ADMIN_ADDR)
        .with_denom(&atom_denom)
        .send()
        .unwrap();

    // Wait for coins to arrive
    thread::sleep(Duration::from_secs(5));

    // liquid stake the ibc'd atoms for stuatom
    test_ctx
        .build_tx_liquid_stake()
        .with_key(ADMIN_KEY)
        .with_amount(amount_to_liquid_stake)
        .with_denom(&atom_denom)
        .send()
        .unwrap();

    // send the stATOM to neutron to eventually add it to the pool
    test_ctx
        .build_tx_transfer()
        .with_chain_name(STRIDE_CHAIN_NAME)
        .with_key(ADMIN_KEY)
        .with_amount(amount_to_liquid_stake)
        .with_recipient(NEUTRON_CHAIN_ADMIN_ADDR)
        .with_denom(NATIVE_STATOM_DENOM)
        .send()
        .unwrap();

    // Send some ATOM as well
    test_ctx
        .build_tx_transfer()
        .with_chain_name(GAIA_CHAIN_NAME)
        .with_amount(50000000)
        .with_recipient(NEUTRON_CHAIN_ADMIN_ADDR)
        .with_denom(&atom_denom)
        .send()
        .unwrap();

    // Get the pool address
    let pair_info = contract_query(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN_NAME),
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
        receiver: Some(NEUTRON_CHAIN_ADMIN_ADDR.to_string()),
    };

    contract_execute(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN_NAME),
        pool_addr,
        DEFAULT_KEY,
        &serde_json::to_string(&provide_liquidity_msg).unwrap(),
        &format!(
            "--amount 10000000{neutron_atom_denom},10000000{neutron_statom_denom} {EXECUTE_FLAGS}"
        ),
    )?;

    thread::sleep(Duration::from_secs(5));

    let code_id_ibc_forwarder = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("valence_ibc_forwarder")
        .unwrap();

    let code_id_single_party_pol_holder = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("valence_single_party_pol_holder")
        .unwrap();

    let code_id_remote_chain_splitter = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("valence_remote_chain_splitter")
        .unwrap();

    let code_id_astroport_liquid_pooler = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("valence_astroport_liquid_pooler")
        .unwrap();

    let code_id_stride_liquid_staker = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("valence_stride_liquid_staker")
        .unwrap();

    let code_id_interchain_router = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("valence_interchain_router")
        .unwrap();

    let code_id_single_party_pol_covenant = *test_ctx
        .get_chain(NEUTRON_CHAIN_NAME)
        .contract_codes
        .get("valence_covenant_single_party_pol")
        .unwrap();

    // Now we can start the covenants
    let chain = localic_std::node::Chain::new(
        test_ctx
            .get_request_builder()
            .get_request_builder(NEUTRON_CHAIN_NAME),
    );
    let current_height = chain.get_height();

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
            liquid_staker_code: code_id_stride_liquid_staker,
            interchain_router_code: code_id_interchain_router,
        },
        clock_tick_max_gas: None,
        lockup_period: Expiration::AtHeight(current_height + 110),
        ls_info: LsInfo {
            ls_denom: NATIVE_STATOM_DENOM.to_string(),
            ls_denom_on_neutron: neutron_statom_denom.to_string(),
            ls_chain_to_neutron_channel_id: test_ctx
                .get_transfer_channels()
                .src(STRIDE_CHAIN_NAME)
                .dest(NEUTRON_CHAIN_NAME)
                .get(),
            ls_neutron_connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN_NAME)
                .dest(STRIDE_CHAIN_NAME)
                .get(),
        },
        ls_forwarder_config: CovenantPartyConfig::Interchain(InterchainCovenantParty {
            party_receiver_addr: NEUTRON_CHAIN_ADMIN_ADDR.to_string(),
            party_chain_connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN_NAME)
                .dest(GAIA_CHAIN_NAME)
                .get(),
            ibc_transfer_timeout: Uint64::new(10000),
            party_to_host_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(GAIA_CHAIN_NAME)
                .dest(STRIDE_CHAIN_NAME)
                .get(),
            host_to_party_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(STRIDE_CHAIN_NAME)
                .dest(GAIA_CHAIN_NAME)
                .get(),
            remote_chain_denom: atom_denom.clone(),
            addr: NEUTRON_CHAIN_ADMIN_ADDR.to_string(),
            native_denom: neutron_atom_denom.clone(),
            contribution: Coin {
                denom: atom_denom.to_string(),
                amount: Uint128::new(10000000),
            },
            denom_to_pfm_map: BTreeMap::new(),
            fallback_address: None,
        }),
        lp_forwarder_config: CovenantPartyConfig::Interchain(InterchainCovenantParty {
            party_receiver_addr: NEUTRON_CHAIN_ADMIN_ADDR.to_string(),
            party_chain_connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN_NAME)
                .dest(GAIA_CHAIN_NAME)
                .get(),
            ibc_transfer_timeout: Uint64::new(10000),
            party_to_host_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(GAIA_CHAIN_NAME)
                .dest(NEUTRON_CHAIN_NAME)
                .get(),
            host_to_party_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(NEUTRON_CHAIN_NAME)
                .dest(GAIA_CHAIN_NAME)
                .get(),
            remote_chain_denom: atom_denom.clone(),
            addr: NEUTRON_CHAIN_ADMIN_ADDR.to_string(),
            native_denom: neutron_atom_denom.clone(),
            contribution: Coin {
                denom: atom_denom.clone(),
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
                .src(NEUTRON_CHAIN_NAME)
                .dest(GAIA_CHAIN_NAME)
                .get(),
            connection_id: test_ctx
                .get_connections()
                .src(NEUTRON_CHAIN_NAME)
                .dest(GAIA_CHAIN_NAME)
                .get(),
            denom: neutron_atom_denom.clone(),
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
                .src(NEUTRON_CHAIN_NAME)
                .dest(GAIA_CHAIN_NAME)
                .get(),
            ibc_transfer_timeout: Uint64::new(300),
            party_to_host_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(GAIA_CHAIN_NAME)
                .dest(NEUTRON_CHAIN_NAME)
                .get(),
            host_to_party_chain_channel_id: test_ctx
                .get_transfer_channels()
                .src(NEUTRON_CHAIN_NAME)
                .dest(GAIA_CHAIN_NAME)
                .get(),
            remote_chain_denom: atom_denom.clone(),
            addr: ACC_1_ADDRESS_NEUTRON.to_string(),
            native_denom: neutron_atom_denom.clone(),
            contribution: Coin {
                denom: atom_denom.clone(),
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
            .get_request_builder(NEUTRON_CHAIN_NAME),
        DEFAULT_KEY,
        code_id_single_party_pol_covenant,
        &serde_json::to_string(&instantiate_covenant_msg).unwrap(),
        "single-party-pol-covenant",
        None,
        "",
    )?;

    // Now that we successfully instantiated the covenant, let's get all the addresses that we will add to our ticker bot so it can start ticking them
    let ntrn_request_builder = test_ctx
        .get_request_builder()
        .get_request_builder(NEUTRON_CHAIN_NAME);

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
    let addresses = vec![
        liquid_staker_address,
        ibc_forwarder_address,
        liquid_pooler_forwarder_address,
        liquid_pooler_address,
        remote_chain_splitter_address,
        interchain_router_address,
        holder_address,
        BOT_ADDRESS,
    ];

    for address in &addresses {
        send(
            test_ctx
                .get_request_builder()
                .get_request_builder(NEUTRON_CHAIN_NAME),
            DEFAULT_KEY,
            address,
            &[Coin {
                denom: "untrn".to_string(),
                amount: Uint128::new(10000000),
            }],
            &Coin {
                denom: "untrn".to_string(),
                amount: Uint128::new(5000),
            },
        )
        .unwrap();
    }

    println!("Bot is now ready to run!");

    Ok(())
}
