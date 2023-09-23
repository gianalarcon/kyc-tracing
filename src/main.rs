use dotenv::dotenv;
use ethers::providers::Http;
use ethers::providers::Middleware;
use ethers::providers::Provider;
use ethers::types::Chain;
use ethers::types::H160;
use ethers::types::H256;
use ethers::types::U64;
use ethers_etherscan::account::TokenQueryOption;
use ethers_etherscan::Client;
use std::collections::VecDeque;
use std::str::FromStr;
//use std::sync::Arc;
//use std::sync::Mutex;
use std::time;
use std::vec;

const USDC_CONTRACT: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
//const USDT_CONTRACT: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";

// 30 days
const BLOCKS_PER_MONTH: u64 = 60 * 60 * 24 * 30 / 12;
//pub struct AppState {
//visited_account_addresses: Mutex<Vec<H160>>,
//base_block_number: U64,
//founded_exchange_accounts: Mutex<VecDeque<H160>>,
//}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set client and read env values and
    let t0 = time::Instant::now();

    dotenv().ok();

    let url = std::env::var("RPC_URL").expect("RPC_URL must be set.");
    let provider = Provider::<Http>::try_from(url)?;

    let last_block_number = provider.get_block_number().await?;
    let base_block_number = last_block_number - BLOCKS_PER_MONTH;
    let tx_hash = "0x631bcb5a618557008bba687cca61d5cc500dc6ba1bf38939d787e165cf3b0133";
    //let tx_hash = "0x2cb65e00cefc611ac21d228ee0e5df008a7fe63a16cb1f300baad609b200a16e";
    println!("Transaction to inspect: {}", tx_hash);
    let h: H256 = H256::from_str(tx_hash)?;

    let tx = provider.get_transaction(h).await?.unwrap();
    let initial_funding_account = tx.from;
    println!(
        "Initial list of detected Funding Accounts: {:#?}",
        initial_funding_account
    );

    //let app_state = Arc::new(AppState {
    //visited_account_addresses: Mutex::new(vec![]),
    //base_block_number,
    //founded_exchange_accounts: Mutex::new(VecDeque::new()),
    //});

    let mut visited_account_addresses: Vec<H160> = vec![];
    let mut founded_exchange_accounts: VecDeque<H160> = VecDeque::new();
    bfs_search(
        base_block_number,
        &mut VecDeque::from(vec![initial_funding_account]),
        &mut visited_account_addresses,
        &mut founded_exchange_accounts,
    )
    .await?;
    println!(
        "Founded Exchange Accounts: {:#?}",
        founded_exchange_accounts
    );
    println!("Time elapsed: {:?}", t0.elapsed().as_secs_f64());
    Ok(())
}

async fn get_funding_accounts(
    base_block_number: U64,
    account_address: H160,
    visited_account_addresses: &[H160],
) -> Result<VecDeque<H160>, Box<dyn std::error::Error>> {
    // --Potential bottleneck
    // Query user_address tranfers with USDC_CONTRACT_ADDRESS
    let t0 = time::Instant::now();
    let token_contract = USDC_CONTRACT.parse::<H160>()?;
    let client =
        Client::new_from_env(Chain::Mainnet).expect("ETHERSCAN_API_KEY must be set on .env file");
    let query = TokenQueryOption::ByAddressAndContract(account_address, token_contract);
    println!("Time elapsed to setup: {:?}", t0.elapsed().as_secs_f64());
    // Retrieve ERC20 token transfer events
    let t0 = time::Instant::now();
    let events = client.get_erc20_token_transfer_events(query, None).await?;
    println!(
        "Time elapsed to retrieve events: {:?}",
        t0.elapsed().as_secs_f64()
    );
    let mut list: VecDeque<H160> = VecDeque::new();
    // --Potential bottleneck
    let t0 = time::Instant::now();
    for event in events.iter() {
        let event_block_number = event.block_number.as_number().unwrap();
        if !list.contains(&event.from)
            && event_block_number > base_block_number
            && event.value != 0.into()
            && event.from != account_address
            && !visited_account_addresses.contains(&event.from)
        {
            list.push_back(event.from);
        }
    }
    println!(
        "Time elapsed to filter events: {:?}",
        t0.elapsed().as_secs_f64()
    );
    Ok(list)
}

async fn bfs_search(
    base_block_number: U64,
    funding_account_addresses: &mut VecDeque<H160>,
    visited_account_addresses: &mut Vec<H160>,
    founded_exchange_accounts: &mut VecDeque<H160>,
) -> Result<(), Box<dyn std::error::Error>> {
    while let Some(account) = funding_account_addresses.pop_front() {
        // Pop up the first element
        println!("-Account to inspect: {}", account);
        visited_account_addresses.push(account);
        if is_exchange(account) {
            founded_exchange_accounts.push_back(account);
            println!(
                "	{} is a Exchange account. Finish search on this branch",
                account
            );
        } else {
            println!(
                "	{} is not a Exchange account. Pop up current account and continue searching",
                account
            );
            let new_from_account_addresses =
                get_funding_accounts(base_block_number, account, visited_account_addresses).await?;
            let mut new_from_account_addresses = new_from_account_addresses
                .into_iter()
                .filter(|account| !funding_account_addresses.contains(account))
                .collect::<VecDeque<H160>>();
            funding_account_addresses.append(&mut new_from_account_addresses); // Add new accounts to the end
            println!("Already Visited accounts {:#?}", visited_account_addresses);
            println!(
                "Updated list of funding accounts to inspect {:#?}",
                funding_account_addresses
            );
        }
    }
    Ok(())
}

fn is_exchange(account: H160) -> bool {
    // Gemini. Binance, Disperse.app account, OnlyDust account, Uniswap V3: USDC 3 account
    let vec = vec![
        "0x5f65f7b609678448494De4C87521CdF6cEf1e932", //agemini 4
        "0x28C6c06298d514Db089934071355E5743bf21d60",
        "0x21a31Ee1afC51d94C2eFcCAa2092aD1028285549",
        "0xD152f549545093347A162Dce210e7293f1452150",
        "0x51f190B6A9CC76BF76BC56C730149604731D4d29",
        "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
    ];
    let vec = vec
        .into_iter()
        .map(|s| s.parse().unwrap())
        .collect::<Vec<H160>>();
    vec.contains(&account)
}
