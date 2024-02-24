use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Debug)]
enum APIErrors {
    AccountNotFound,
    TransactionLimitExceeded,
}

impl IntoResponse for APIErrors {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            APIErrors::AccountNotFound => (StatusCode::NOT_FOUND, "Account not found"),
            APIErrors::TransactionLimitExceeded => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Transaction limit exceeded",
            ),
        };

        (status, message).into_response()
    }
}

#[derive(Clone)]
struct ClientAccount {
    balance: i64,
    limit: i64,
    transactions: Vec<Transaction>,
}

// Buffer with size 10 for Vec<Transaction>
struct AuxBuffer(Vec<Transaction>);

impl AuxBuffer {
    fn new() -> Self {
        Self(Vec::with_capacity(10))
    }

    fn push(&mut self, transaction: Transaction) {
        if self.0.len() == 10 {
            self.0.remove(0);
        }
        self.0.push(transaction);
    }
}

impl From<Vec<Transaction>> for AuxBuffer {
    fn from(transactions: Vec<Transaction>) -> Self {
        let mut buffer = Self::new();
        for transaction in transactions {
            buffer.push(transaction);
        }
        buffer
    }
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
    Json(transaction_request): Json<TransactionRequest>,
) -> Result<Json<impl serde::Serialize>, APIErrors> {
    let mut accounts = accounts.lock().await;
    let account = accounts
        .get_mut(&account_id)
        .ok_or(APIErrors::AccountNotFound)?;

    let adjustment = match transaction_request.type_ {
        TransactionType::Credit => transaction_request.value,
        TransactionType::Debit => -transaction_request.value,
    };

    let balance_after_transaction = account.balance + adjustment;

    if balance_after_transaction < -account.limit {
        return Err(APIErrors::TransactionLimitExceeded);
    }

    account.balance += adjustment;
    account.transactions.push(Transaction {
        value: transaction_request.value,
        type_: transaction_request.type_,
        description: transaction_request.description.clone(),
        date: OffsetDateTime::now_utc(),
    });

    Ok(Json(json!({
        "balance": account.balance,
        "limit": account.limit,
    })))
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
