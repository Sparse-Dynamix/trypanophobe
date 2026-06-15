use salvo::oapi::swagger_ui::SwaggerUi;
use salvo::oapi::OpenApi;
use salvo::prelude::*;

pub const OPENAPI_JSON_PATH: &str = "/api-doc/openapi.json";
pub const SWAGGER_UI_PATH: &str = "/swagger-ui";

pub fn mount_openapi(router: Router) -> Router {
    let doc = OpenApi::new("trypanophobe filter API", "1.0.0").merge_router(&router);

    Router::new()
        .get(root_redirect)
        .push(router)
        .unshift(doc.into_router(OPENAPI_JSON_PATH))
        .unshift(SwaggerUi::new(OPENAPI_JSON_PATH).into_router(SWAGGER_UI_PATH))
}

#[handler]
async fn root_redirect() -> Redirect {
    Redirect::temporary(SWAGGER_UI_PATH)
}
