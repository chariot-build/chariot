use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub mod events;
pub mod jobs;
pub mod ledger;
pub mod meta;

pub struct ApiError {
    status: StatusCode,
    error: Option<anyhow::Error>,
}

impl ApiError {
    pub fn new(status: StatusCode) -> Self {
        Self { status, error: None }
    }

    pub fn from_error(status: StatusCode, error: impl Into<anyhow::Error>) -> Self {
        Self {
            status,
            error: Some(error.into()),
        }
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND)
    }

    pub fn bad_request(error: impl Into<anyhow::Error>) -> Self {
        Self::from_error(StatusCode::BAD_REQUEST, error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            if let Some(err) = self.error {
                eprintln!("{:#}", err);
            }

            (self.status, "internal server error").into_response()
        } else {
            match self.error {
                Some(err) => (self.status, format!("{}", err)).into_response(),
                None => self.status.into_response(),
            }
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self::from_error(StatusCode::INTERNAL_SERVER_ERROR, err)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
