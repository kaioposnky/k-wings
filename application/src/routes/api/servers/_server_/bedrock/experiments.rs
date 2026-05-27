use crate::response::{ApiResponse, ApiResponseResult};
use crate::routes::api::servers::_server_::GetServer;
use crate::server::bedrock::services::world_data;
use crate::server::bedrock::types::experiment::Experiment;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub mod get {
    use super::*;

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
        experiments: Vec<Experiment>,
    }

    #[utoipa::path(get, path = "/experiments", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = crate::routes::ApiError),
    ), params(
        ("server" = uuid::Uuid, description = "The server uuid"),
    ))]
    pub async fn route(server: GetServer) -> ApiResponseResult {
        let (root, filesystem) = server
            .filesystem
            .resolve_readable_fs(&server, std::path::Path::new(""))
            .await;

        let (_, data) = world_data::read_server_level_dat(&server, filesystem.as_ref(), &root)
            .await
            .map_err(|_| {
                ApiResponse::error("World not created yet").with_status(StatusCode::BAD_REQUEST)
            })?;

        let experiments = world_data::get_experiments(&data.root_tag);

        ApiResponse::new_serialized(Response {
            message: "Experiments retrieved successfully".to_string(),
            experiments,
        })
        .ok()
    }
}

pub mod post {
    use super::*;
    use axum::Json;

    #[derive(ToSchema, Deserialize)]
    pub struct UpdateExperimentsRequest {
        experiments: Vec<Experiment>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
        updated: Vec<Experiment>,
    }

    #[utoipa::path(post, path = "/experiments", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = crate::routes::ApiError),
    ), params(
        ("server" = uuid::Uuid, description = "The server uuid"),
    ), request_body = UpdateExperimentsRequest)]
    pub async fn route(
        server: GetServer,
        Json(body): Json<UpdateExperimentsRequest>,
    ) -> ApiResponseResult {
        let (root, filesystem) = server
            .filesystem
            .resolve_writable_fs(&server, std::path::Path::new(""))
            .await;

        let (world_path, mut data) =
            world_data::read_server_level_dat(&server, filesystem.as_ref(), &root)
                .await
                .map_err(|_| {
                    ApiResponse::error("World not created yet").with_status(StatusCode::BAD_REQUEST)
                })?;

        let updated = world_data::update_experiments(&mut data.root_tag, &body.experiments);

        world_data::save_server_level_dat(filesystem.as_ref(), &root, &world_path, &data)
            .await
            .map_err(|e| ApiResponse::error(&format!("Failed to save level.dat: {}", e)))?;

        ApiResponse::new_serialized(Response {
            message: "Experiments updated successfully".to_string(),
            updated,
        })
        .ok()
    }
}
