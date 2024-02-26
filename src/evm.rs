use crate::db::{self, Token};
use anyhow::Result;
use ethers::{prelude::ProviderError::JsonRpcClientError, prelude::*, utils::hex::ToHex};
use sqlx::PgPool;
use std::{env, sync::Arc};
use tokio::sync::mpsc;

pub async fn create_provider() -> Result<Arc<Provider<Ws>>> {
    let provider = Provider::<Ws>::connect(env::var("RPC_URL_WS")?).await?;
    Ok(Arc::new(provider))
}

pub async fn update_db(
    connection_pool: PgPool,
    provider: Arc<Provider<Ws>>,
    mut rx: mpsc::Receiver<Token>,
) -> Result<()> {
    let mut token_count = db::all_token_count(&connection_pool).await?;

    if token_count == 0 {
        token_count = add_start_tokens(&connection_pool).await?;
    }

    let mut set = tokio::task::JoinSet::new();
    let tokens = db::all_token(&connection_pool, 0, token_count, None).await?;

    for token in tokens.iter() {
        set.spawn(add_balances_by_token(
            connection_pool.clone(),
            provider.clone(),
            token.clone(),
        ));
    }

    loop {
        tokio::select! {
            Some(res) = set.join_next() => res??, //TODO: Check if its skip err when spawn new task
            Some(token) = rx.recv() => {
                set.spawn(add_balances_by_token(
                    connection_pool.clone(),
                    provider.clone(),
                    token.clone(),
                ));
            }
        }
    }
}

async fn add_start_tokens(connection_pool: &PgPool) -> Result<i64> {
    let addresses = vec![
        "50327c6c5a14DCaDE707ABad2E27eB517df87AB5", //trx 24,352
        "582d872A1B094FC48F5DE31D3B73F2D9bE47def1", //toncoin 94,646
        "2AF5D2aD76741191D15Dfe7bF6aC92d4Bd912Ca3", //leo 41,588
        // "e28b3B32B6c345A34Ff64674606124Dd5Aceca30", //inj 493,857
        "c5f0f7b66764F6ec8C8Dff7BA683102295E16409", //fdusd 3,976
    ];

    for contract_addr in addresses.iter() {
        add_token_by_contract(connection_pool, contract_addr).await?;
    }

    Ok(addresses.len() as i64)
}

pub async fn add_token_by_contract(connection_pool: &PgPool, contract_addr: &str) -> Result<Token> {
    let symbol = "TEST";
    let decimals = 6;

    let token_id = db::add_token(connection_pool, contract_addr, &-1, symbol, &decimals).await?;

    Ok(Token {
        id: token_id,
        contract_addr: contract_addr.to_string(),
        last_checked_block: -1,
        symbol: symbol.to_string(),
        decimals,
    })
}

async fn add_balances_by_token(
    connection_pool: PgPool,
    provider: Arc<Provider<Ws>>,
    mut token: Token,
) -> Result<()> {
    let mut last_block: i64 = provider.get_block_number().await?.as_u32().into();

    let mut from = token.last_checked_block + 1;
    let mut step = 1_000_000;

    if from + step > last_block {
        step = last_block - from;
    }

    let mut filter = Filter::new()
        .address(token.contract_addr.parse::<Address>()?)
        .event("Transfer(address,address,uint256)");

    while from < last_block {
        filter = filter.select(from..from + step);

        let logs = provider.get_logs(&filter).await;

        match logs {
            Ok(logs) => {
                for log in logs.iter() {
                    upsert_balance_from_log(&connection_pool, log, &token.id).await?;
                }

                from += step;

                tracing::debug!(
                    "Update: Token: {}; Block: {}; Logs: {};",
                    token.contract_addr,
                    from,
                    logs.len()
                );

                db::update_token_last_checked_block(&connection_pool, &from, &token.id).await?;
                token.last_checked_block = from;
                from += 1;

                step = match logs.len() {
                    0..=100 => step * 2,
                    101..=1000 => step + step / 2,
                    1001..=5000 => step + step / 4,
                    _ => step,
                };

                if from + step > last_block {
                    let actual_last_block: i64 = provider.get_block_number().await?.as_u32().into();
                    if actual_last_block - last_block > 10 {
                        last_block = actual_last_block;
                    }
                    step = last_block - from;
                }
            }
            Err(JsonRpcClientError(err)) if err.to_string().contains("code: -32005") => {
                tracing::debug!("Too Many: Token: {}; Block: {};", token.contract_addr, from);

                step /= 3;

                if step == 0 {
                    step = 1000;
                }
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    log_listener(connection_pool, provider, token).await?;

    Ok(())
}

async fn upsert_balance_from_log(
    connection_pool: &PgPool,
    log: &Log,
    token_id: &i32,
) -> Result<()> {
    if log.topics[1] != log.topics[2] {
        let from_holder_id = db::add_or_get_holder(
            connection_pool,
            &Address::from(log.topics[1]).encode_hex_upper::<String>(),
        )
        .await?;

        let to_holder_id = db::add_or_get_holder(
            connection_pool,
            &Address::from(log.topics[2]).encode_hex_upper::<String>(),
        )
        .await?;

        let amount = U256::from_big_endian(&log.data).to_string();

        db::upsert_balance(
            connection_pool,
            &from_holder_id,
            &to_holder_id,
            &token_id,
            &amount,
        )
        .await?;
    }

    Ok(())
}

pub async fn log_listener(
    connection_pool: PgPool,
    provider: Arc<Provider<Ws>>,
    token: Token,
) -> Result<()> {
    tracing::debug!(
        "Start Listen: Token: {}; Block: {}/{}",
        token.contract_addr,
        token.last_checked_block,
        provider.get_block_number().await?
    );

    let filter = Filter::new()
        .address(token.contract_addr.parse::<Address>()?)
        .event("Transfer(address,address,uint256)")
        .select((token.last_checked_block + 1)..);

    let mut stream = provider.subscribe_logs(&filter).await?;

    while let Some(log) = stream.next().await {
        tracing::debug!(
            "NewLog: Token: {}; Block: {:?}; Hash: {:?};",
            token.contract_addr,
            &log.block_number,
            &log.transaction_hash
        );

        if let Some(block_num) = log.block_number {
            db::update_token_last_checked_block(
                &connection_pool,
                &block_num.as_u32().into(),
                &token.id,
            )
            .await?;

            upsert_balance_from_log(&connection_pool, &log, &token.id).await?;
        }
    }

    Ok(())
}
