use async_trait::async_trait;
use salvo::http::{ParseError, StatusCode};
use salvo::oapi::{Components, EndpointOutRegister, Operation, Response as OapiResponse, ToSchema};
use salvo::prelude::{Depot, Request, Response};
use salvo::writing::{Json, Writer};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("payload too large: {0}")]
    PayloadTooLarge(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("unprocessable: {0}")]
    Unprocessable(String),
    #[error("dependency timeout: {0}")]
    DependencyTimeout(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::DependencyTimeout(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn body(&self) -> ErrorBody {
        match self {
            Self::DependencyTimeout(dep) => ErrorBody {
                error: "dependency_timeout".into(),
                dependency: Some(dep.clone()),
                message: None,
            },
            Self::Forbidden(msg) => ErrorBody {
                error: msg.clone(),
                dependency: None,
                message: None,
            },
            other => ErrorBody {
                error: other.to_string(),
                dependency: None,
                message: None,
            },
        }
    }
}

impl From<ParseError> for AppError {
    fn from(value: ParseError) -> Self {
        Self::BadRequest(value.to_string())
    }
}

impl EndpointOutRegister for AppError {
    fn register(components: &mut Components, operation: &mut Operation) {
        let schema = ErrorBody::to_schema(components);
        for (code, summary) in [
            ("400", "Bad request"),
            ("403", "Forbidden"),
            ("413", "Payload too large"),
            ("422", "Unprocessable"),
            ("503", "Dependency unavailable"),
            ("500", "Internal error"),
        ] {
            operation.responses.insert(
                code,
                OapiResponse::new(summary).add_content("application/json", schema.clone()),
            );
        }
    }
}

#[async_trait]
impl Writer for AppError {
    async fn write(self, _req: &mut Request, _depot: &mut Depot, res: &mut Response) {
        res.status_code(self.status());
        if matches!(self, Self::DependencyTimeout(_)) {
            res.headers_mut()
                .insert("Retry-After", "5".parse().unwrap());
        }
        res.render(Json(self.body()));
    }
}

pub type AppResult<T> = Result<T, AppError>;
