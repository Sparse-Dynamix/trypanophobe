pub mod filter;
pub mod health;

use std::sync::Arc;

use salvo::Depot;

use crate::state::AppState;

pub(crate) fn app_state(depot: &Depot) -> Arc<AppState> {
    Arc::clone(
        depot
            .obtain::<Arc<AppState>>()
            .expect("AppState not injected"),
    )
}
