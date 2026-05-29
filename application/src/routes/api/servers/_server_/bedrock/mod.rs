use crate::routes::State;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

mod experiments;
mod world_version;
mod education;
mod packages_manifest;
mod packs_ordered;
mod pack_order;
mod pack_delete;
mod packs_enabled;
mod install_package;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(experiments::get::route))
        .routes(routes!(experiments::post::route))
        .routes(routes!(world_version::get::route))
        .routes(routes!(education::get::route))
        .routes(routes!(education::post::route))
        .routes(routes!(packages_manifest::get::route))
        .routes(routes!(packs_ordered::get::route))
        .routes(routes!(pack_order::post::route))
        .routes(routes!(pack_delete::post::route))
        .routes(routes!(packs_enabled::get::route))
        .routes(routes!(install_package::post::route))
        .with_state(state.clone())
}
