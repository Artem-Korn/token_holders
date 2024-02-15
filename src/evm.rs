use crate::db::{self, update_token_last_checked_block, Token};
use anyhow::Result;
use ethers::{
    prelude::{ProviderError::JsonRpcClientError, *},
    utils::hex::ToHex,
};
use sqlx::PgPool;
use std::{collections::HashMap, env};

pub async fn create_provider() -> Result<Provider<Ws>> {
    let provider = Provider::<Ws>::connect(env::var("RPC_URL_WS")?).await?;
    Ok(provider)
}

pub async fn update_db(connection_pool: &PgPool, provider: &Provider<Ws>) -> Result<()> {
    let token_count = db::all_token_count(connection_pool).await?;

    if token_count == 0 {
        fill_db(connection_pool, provider).await?;
    } else {
        // let last_block = provider.get_block_number().await?.as_u32().into();

        // let tokens = db::all_token(connection_pool, 0, token_count, None).await?;

        // for token in tokens.iter() {
        //     if token.last_checked_block < last_block {
        //         add_balances_by_token(
        //             connection_pool,
        //             provider,
        //             &token.contract_addr,
        //             &token.id,
        //             &last_block,
        //             &token.last_checked_block,
        //         )
        //         .await?;
        //     }
        // }
        todo!();
    }

    Ok(())
}

async fn fill_db(connection_pool: &PgPool, provider: &Provider<Ws>) -> Result<()> {
    let addresses = vec![
        "50327c6c5a14DCaDE707ABad2E27eB517df87AB5", //trx 24,352
                                                    // "582d872A1B094FC48F5DE31D3B73F2D9bE47def1", //toncoin 94,646
                                                    // "2AF5D2aD76741191D15Dfe7bF6aC92d4Bd912Ca3", //leo 41,588
                                                    // "e28b3B32B6c345A34Ff64674606124Dd5Aceca30", //inj 493,857
                                                    // "c5f0f7b66764F6ec8C8Dff7BA683102295E16409", //fdusd 3,976
    ];

    let mut tokens_map: HashMap<H160, i32> = HashMap::new();

    // let last_checked_block = provider.get_block_number().await?.as_u32().into();
    let last_checked_block = 19_000_000;

    for contract_addr in addresses.iter() {
        let token = add_token_by_contract(
            connection_pool,
            provider,
            contract_addr,
            &last_checked_block,
        )
        .await?;

        tokens_map.insert(token.contract_addr.parse::<Address>()?, token.id);
    }

    log_listener(connection_pool, provider, tokens_map, &last_checked_block).await?;

    Ok(())
}

async fn add_token_by_contract(
    connection_pool: &PgPool,
    provider: &Provider<Ws>,
    contract_addr: &str,
    last_checked_block: &i64,
) -> Result<Token> {
    let symbol = "NTKN";
    let decimals = 12;

    let token_id = db::add_token(
        connection_pool,
        contract_addr,
        last_checked_block,
        symbol,
        &decimals,
    )
    .await?;

    add_balances_by_token(
        connection_pool,
        provider,
        &contract_addr,
        &token_id,
        &last_checked_block,
        &0,
    )
    .await?;

    Ok(Token {
        id: token_id,
        contract_addr: contract_addr.to_string(),
        last_checked_block: last_checked_block.clone(),
        symbol: symbol.to_string(),
        decimals,
    })
}

async fn add_balances_by_token(
    connection_pool: &PgPool,
    provider: &Provider<Ws>,
    contract_addr: &str,
    token_id: &i32,
    last_block: &i64,
    start: &i64,
) -> Result<()> {
    let last_block = last_block.clone();
    let mut from = start.clone();
    let mut step = 1_000_000;

    let mut counter = 0;

    let mut filter = Filter::new()
        .address(contract_addr.parse::<Address>()?)
        .event("Transfer(address,address,uint256)");

    while from < last_block {
        filter = filter.select(from..from + step);

        let logs = provider.get_logs(&filter).await;

        match logs {
            Ok(logs) => {
                let logs_len = logs.len();

                println!(
                    "({}) [{} + {}] - OK, LOGS: {}",
                    contract_addr, from, step, logs_len
                );

                for log in logs.iter() {
                    upsert_balance_from_log(connection_pool, log, token_id).await?;
                }

                counter += logs_len;

                from += step;

                step = match logs_len {
                    0..=100 => step * 2,
                    101..=1000 => step + step / 2,
                    1001..=5000 => step + step / 4,
                    _ => step,
                };

                if from + step > last_block {
                    step = last_block - from;
                }
            }
            Err(JsonRpcClientError(err)) if err.to_string().contains("code: -32005") => {
                println!("({}) [{} + {}] - TOO MANY LOGS", contract_addr, from, step);

                step /= 3;

                if step == 0 {
                    step = 1000;
                }
            }
            Err(err) => {
                println!("({}) [{} + {}] ERR: {}", contract_addr, from, step, err);
                return Err(err.into());
            }
        }
    }

    println!("RESULT: {counter}");

    Ok(())
}

async fn upsert_balance_from_log(
    connection_pool: &PgPool,
    log: &Log,
    token_id: &i32,
) -> Result<()> {
    let from_holder_id =
        db::add_or_get_holder(connection_pool, &log.topics[1].encode_hex_upper::<String>()).await?;

    let to_holder_id =
        db::add_or_get_holder(connection_pool, &log.topics[2].encode_hex_upper::<String>()).await?;

    let amount = U256::from_big_endian(&log.data).to_string();

    db::upsert_balance(
        connection_pool,
        &from_holder_id,
        &to_holder_id,
        &token_id,
        &amount,
    )
    .await?;

    Ok(())
}

pub async fn log_listener(
    connection_pool: &PgPool,
    provider: &Provider<Ws>,
    tokens_map: HashMap<H160, i32>,
    last_checked_block: &i64,
) -> Result<()> {
    let addresses: Vec<H160> = tokens_map.clone().into_keys().collect();

    let filter = Filter::new()
        .address(addresses)
        .event("Transfer(address,address,uint256)")
        .select(last_checked_block.clone()..);

    let mut stream = provider.subscribe_logs(&filter).await?;

    while let Some(log) = stream.next().await {
        if let Some(token_id) = tokens_map.get(&log.address) {
            upsert_balance_from_log(connection_pool, &log, token_id).await?;
            update_token_last_checked_block(
                connection_pool,
                &log.block_number.unwrap().as_u32().into(),
                token_id,
            )
            .await?;

            println!(
                "Number: {}, From: {} To: {}",
                &log.block_number.unwrap(),
                &log.topics[1],
                &log.topics[2]
            );
        }
    }

    Ok(())
}
