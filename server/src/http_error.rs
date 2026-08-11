use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug)]
pub struct HttpError {
    status: StatusCode,
    message: String,
    retryable: bool,
    code: Option<&'static str>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
}

impl HttpError {
    pub fn new(status: StatusCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            status,
            message: message.into(),
            retryable,
            code: None,
        }
    }

    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message, false)
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
                retryable: self.retryable,
                code: self.code,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::HttpError;
    use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};

    #[tokio::test]
    async fn serializes_the_shared_json_error_contract() {
        let response =
            HttpError::new(StatusCode::BAD_GATEWAY, "upstream failed", true).into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read error response body");
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("parse error response body");
        assert_eq!(
            value,
            serde_json::json!({
                "error": "upstream failed",
                "retryable": true,
            })
        );
    }

    #[tokio::test]
    async fn serializes_an_optional_machine_readable_error_code() {
        let response = HttpError::new(StatusCode::CONFLICT, "state changed", true)
            .with_code("favorite_order_stale")
            .into_response();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read error response body");
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("parse error response body");
        assert_eq!(value["code"], "favorite_order_stale");
        assert_eq!(value["retryable"], true);
    }
}
