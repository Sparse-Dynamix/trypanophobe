use salvo::oapi::endpoint;
use salvo::prelude::*;

use crate::error::{AppError, AppResult};
use crate::pipeline::respond::ResponseFormat;
use crate::pipeline::{run_filter, FilterRequest};
use crate::routes::app_state;

async fn handle_filter(req: &mut Request, depot: &mut Depot, res: &mut Response) -> AppResult<()> {
    let state = app_state(depot);

    let url = req
        .queries()
        .get("url")
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("url query parameter is required".into()))?
        .to_string();

    let format_param = req.queries().get("format").map(|s| s.as_str());
    let response_format = ResponseFormat::from_query(format_param)?;

    let content_type = req
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let body = req
        .payload()
        .await
        .map_err(crate::error::AppError::from)?
        .to_vec();

    let outcome = run_filter(
        &state,
        FilterRequest {
            body,
            url,
            content_type,
            response_format,
        },
    )
    .await?;

    outcome.apply_to_response(res);
    Ok(())
}

#[endpoint]
pub async fn filter_post(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> AppResult<()> {
    handle_filter(req, depot, res).await
}

#[endpoint]
pub async fn filter_root(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> AppResult<()> {
    handle_filter(req, depot, res).await
}
