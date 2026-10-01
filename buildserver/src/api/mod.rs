use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub mod ledger;
pub mod meta;

pub struct ApiError {
    status: StatusCode,
    error: anyhow::Error,
}

impl ApiError {
    pub fn new(status: StatusCode, error: impl Into<anyhow::Error>) -> Self {
        Self { status, error: error.into() }
    }

    pub fn bad_request(error: impl Into<anyhow::Error>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            eprintln!("{:#}", self.error); // TODO: handle better
            (self.status, "internal server error").into_response()
        } else {
            (self.status, format!("{}", self.error)).into_response()
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, err)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
