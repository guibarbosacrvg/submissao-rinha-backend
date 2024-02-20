use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Clone)]
struct ClientAccount {
    balance: i64,
    limit: i64,
    transactions: Vec<Transaction>,
}

impl ClientAccount {
    pub fn default_with_limit(limit: i64) -> Self {
        Self {
            balance: 0,
            limit,
            transactions: Vec::with_capacity(10),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
enum TransactionType {
    #[serde(rename = "c")]
    Credit,
    #[serde(rename = "d")]
    Debit,
}

#[derive(Clone, Serialize)]
struct Transaction {
    #[serde(rename = "valor")]
    value: i64,
    #[serde(rename = "tipo")]
    type_: TransactionType,
    #[serde(rename = "descricao")]
    description: String,
    #[serde(rename = "realizada_em", with = "time::serde::rfc3339")]
    date: OffsetDateTime,
}

#[derive(Clone, Deserialize)]
struct TransactionRequest {
    #[serde(rename = "valor")]
    value: i64,
    #[serde(rename = "tipo")]
    type_: TransactionType,
    #[serde(rename = "descricao")]
    description: String,
}

#[tokio::main]
async fn main() {
    let accounts: Arc<Mutex<HashMap<i32, ClientAccount>>> =
        Arc::new(Mutex::new(HashMap::from_iter([
            (1, ClientAccount::default_with_limit(100_000)),
            (2, ClientAccount::default_with_limit(80_000)),
            (3, ClientAccount::default_with_limit(1_000_000)),
            (4, ClientAccount::default_with_limit(10_000_000)),
            (5, ClientAccount::default_with_limit(500_000)),
        ])));

    let app: Router = Router::new()
        .route("/clientes/:id/transacoes", post(transaction))
        .route("/clientes/:id/extrato", get(extract))
        .with_state(accounts);

    let listener: tokio::net::TcpListener =
        tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn transaction(
    Path(account_id): Path<i32>,
    State(accounts): State<Arc<Mutex<HashMap<i32, ClientAccount>>>>,
    Json(transaction): Json<TransactionRequest>,
) -> impl IntoResponse {
    let mut accounts = accounts.lock().await;

    if transaction.value > accounts.get_mut(&account_id).unwrap().limit {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    if transaction.type_ == TransactionType::Debit {
        let tmp_result: i64 = accounts.get_mut(&account_id).unwrap().balance - transaction.value;
        if tmp_result < accounts.get_mut(&account_id).unwrap().limit {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    // Uptade account balance
    let new_balance: i64 = match transaction.type_ {
        TransactionType::Credit => {
            accounts.get_mut(&account_id).unwrap().balance + transaction.value
        }
        TransactionType::Debit => {
            accounts.get_mut(&account_id).unwrap().balance - transaction.value
        }
    };
    
    accounts.get_mut(&account_id).unwrap().balance = new_balance;

    match accounts.get_mut(&account_id) {
        Some(account) => {
            account.transactions.push(Transaction {
                value: transaction.value,
                type_: transaction.type_,
                description: transaction.description,
                date: OffsetDateTime::now_utc(),
            });
            Ok(Json(json!({
                "limite" : account.limit,
                "saldo" : account.balance
            })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn extract(
    Path(account_id): Path<i32>,
    State(accounts): State<Arc<Mutex<HashMap<i32, ClientAccount>>>>,
) -> impl IntoResponse {
    let mut accounts = accounts.lock().await;

    match accounts.get_mut(&account_id) {
        Some(account) => Ok(Json(json!({
            "saldo": {
                "total": account.balance,
                "limite": account.limit,
            },
            "ultimas_transacoes": account.transactions,
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}