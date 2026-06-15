use salvo::oapi::{endpoint, ToSchema};
use salvo::prelude::*;
use serde::Serialize;

use crate::routes::app_state;

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub sentinel: bool,
    pub pihole: bool,
    pub nsfw_text: bool,
    pub nsfw_image: bool,
    pub wolf: bool,
    pub ocr: bool,
}

#[endpoint(responses((status_code = 200, body = HealthResponse)))]
pub async fn health(depot: &mut Depot) -> Json<HealthResponse> {
    let app = app_state(depot);
    let sentinel = *app.readiness.sentinel.borrow();
    let pihole = *app.readiness.pihole.borrow();
    let nsfw_text = *app.readiness.nsfw_text.borrow();
    let nsfw_image = *app.readiness.nsfw_image.borrow();
    let wolf = *app.readiness.wolf.borrow();
    let ocr = *app.readiness.ocr.borrow();
    Json(HealthResponse {
        status: "ok".into(),
        sentinel,
        pihole,
        nsfw_text,
        nsfw_image,
        wolf,
        ocr,
    })
}
