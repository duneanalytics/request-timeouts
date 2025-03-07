use bytes::Bytes;
use log::{info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::time::Duration;
use tokio::task::JoinSet;
use tracing::instrument;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

#[derive(Deserialize, Debug)]
pub struct JsonRpcResponse<T> {
    pub result: T,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: i64,
    pub method: String,
    pub params: Vec<Value>,
}

impl JsonRpcRequest {
    pub fn new(method: String, params: Vec<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id: 1,
            method,
            params,
        }
    }
}

async fn get_block(client: Client, endpoint: String, block_number: i64) {
    call(
        client,
        endpoint,
        "eth_getBlockByNumber".to_string(),
        vec![
            Value::String(format!("0x{:x}", block_number)),
            Value::Bool(true),
        ],
    )
    .await;
}

async fn get_receipts(client: Client, endpoint: String, block_number: i64) {
    call(
        client,
        endpoint,
        "eth_getBlockReceipts".to_string(),
        vec![Value::String(format!("0x{:x}", block_number))],
    )
    .await;
}

async fn get_latest_block(client: Client, endpoint: String) -> i64 {
    let bytes = call(client, endpoint, "eth_blockNumber".to_string(), vec![])
        .await
        .expect("Failed to get block number");
    let response: JsonRpcResponse<String> =
        serde_json::from_slice(&bytes).expect("Failed to parse");
    i64::from_str_radix(&response.result[2..], 16).expect("Failed to parse")
}

#[instrument(level = "info", skip(client))]
async fn call(
    client: Client,
    endpoint: String,
    method: String,
    params: Vec<Value>,
) -> Option<Bytes> {
    let response = client
        .post(endpoint)
        .json(&JsonRpcRequest::new(method.clone(), params))
        .send()
        .await;

    let response = match response {
        Ok(response) => response,
        Err(err) => {
            warn!("Failed request: {:?}", err);
            return None;
        }
    };
    info!("Got response: {:?}", response.status());

    let bytes = response.bytes().await;
    match bytes {
        Ok(bytes) => {
            info!("Got bytes: {:?}", bytes.len());
            Some(bytes)
        }
        Err(err) => {
            warn!("Failed to get bytes: {:?}", err);
            None
        }
    }
}

#[tokio::main]
async fn main() {
    let endpoint: String = env::var("ENDPOINT").expect("ENDPOINT is required");

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_filter(EnvFilter::from_default_env());
    tracing_subscriber::registry().with(fmt_layer).init();

    let concurrency = 10;
    let client = reqwest::ClientBuilder::new()
        .gzip(true)
        .connection_verbose(true)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("Failed to build client");

    warn!("Start!");

    let latest_block = get_latest_block(client.clone(), endpoint.clone()).await;
    let mut pending_blocks = (latest_block - 44_000..latest_block).collect::<Vec<i64>>();
    let mut executing_blocks = JoinSet::new();
    while !pending_blocks.is_empty() || !executing_blocks.is_empty() {
        let endpoint = endpoint.clone();
        if let Some(block_number) = pending_blocks.pop() {
            let client = client.clone();
            executing_blocks.spawn(tokio::spawn(async move {
                get_block(client.clone(), endpoint.clone(), block_number).await;
                get_receipts(client.clone(), endpoint.clone(), block_number).await;
            }));
        }
        info!(
            "Pending: {}, Executing: {}",
            pending_blocks.len(),
            executing_blocks.len()
        );

        while executing_blocks.len() > concurrency || pending_blocks.is_empty() {
            executing_blocks
                .join_next()
                .await
                .expect("Failed to join next")
                .expect("Failed to join")
                .expect("Failed to get result");
        }
    }
}
