use actix_web::{HttpResponse, ResponseError};
use derive_more::Display;
use sqlx::{error::Error as SqlxError, QueryBuilder}; // Import the Display derive

#[derive(Debug, Display)]
pub enum CustomError {
    #[display(fmt = "Data type error: {}", _0)]
    DataTypeError(SqlxError),
    #[display(fmt = "Query error: {}", _0)]
    QueryError(SqlxError),
    #[display(fmt = "Index vector malformed: {}", _0)]
    IndexVectorParseError(serde_json::Error),
    #[display(fmt = "Not enough items returned from query: {}", _0)]
    InsufficientItemsReturned(String),
    #[display(fmt = "Card with id {} not found inside index vector", _0)]
    CardNotFound(i64),
    // #[display(fmt = "Custom error: {}", _0)]
    // Other(String),
}

impl ResponseError for CustomError {
    fn error_response(&self) -> HttpResponse {
        println!("Error during request: {:?}", self);
        HttpResponse::InternalServerError().body("Internal Server Error")
    }
}

pub trait InIndexVector {
    fn in_index_vector(self, idxs: &[i64]) -> Self;
}

impl<T: sqlx::Database> InIndexVector for QueryBuilder<'_, T> {
    fn in_index_vector(mut self, idxs: &[i64]) -> Self {
        if idxs.is_empty() {
            return self;
        }

        let values: Vec<String> = idxs.iter().map(|n| format!("'{n}'")).collect();
        self.push(format!(" IN ({}) ", values.join(",")));
        self
    }
}

pub trait Helper {
    type Output;

    fn ensure_data_type(self) -> Result<Self::Output, CustomError>;
    fn ensure_query_success(self) -> Result<Self::Output, CustomError>;
}

impl<T> Helper for Result<T, sqlx::Error> {
    type Output = T;

    fn ensure_data_type(self) -> Result<T, CustomError> {
        self.map_err(CustomError::DataTypeError)
    }
    fn ensure_query_success(self) -> Result<Self::Output, CustomError> {
        self.map_err(CustomError::QueryError)
    }
}

pub trait ParseIndexVector {
    fn parse_index_vector(self) -> Result<Vec<i64>, CustomError>;
}

impl ParseIndexVector for String {
    fn parse_index_vector(self) -> Result<Vec<i64>, CustomError> {
        serde_json::from_str(&self).map_err(CustomError::IndexVectorParseError)
    }
}

pub trait RemoveCard {
    fn remove_card(self, card_id: i64) -> Result<Vec<i64>, CustomError>;
}

impl RemoveCard for Vec<i64> {
    fn remove_card(mut self, card_id: i64) -> Result<Vec<i64>, CustomError> {
        let Some(position) = self.iter().position(|&id| id == card_id) else {
            return Err(CustomError::CardNotFound(card_id));
        };
        self.remove(position);

        Ok(self)
    }
}

pub trait ToJson {
    fn to_json(self) -> String;
}

impl ToJson for Vec<i64> {
    fn to_json(self) -> String {
        serde_json::to_string(&self).unwrap().clone()
    }
}
