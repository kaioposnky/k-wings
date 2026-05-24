pub mod get {
    use crate::response::{ApiResponse, ApiResponseResult};
    use crate::routes::api::servers::_server_::GetServer;
    use crate::server::bedrock::services::world_data;
    use axum::http::StatusCode;
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
        enabled: bool,
    }

    #[utoipa::path(get, path = "/education", responses(
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

        let enabled = world_data::get_education_features_enabled(&data.root_tag);

        ApiResponse::json(Response {
            message: "Education features status retrieved".to_string(),
            enabled,
        })
        .ok()
    }
}

pub mod post {
    use crate::response::{ApiResponse, ApiResponseResult};
    use crate::routes::api::servers::_server_::GetServer;
    use crate::server::bedrock::services::world_data;
    use axum::http::StatusCode;
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
        enabled: bool,
    }

    #[utoipa::path(post, path = "/education/toggle", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = crate::routes::ApiError),
    ), params(
        ("server" = uuid::Uuid, description = "The server uuid"),
    ))]
    pub async fn route(server: GetServer) -> ApiResponseResult {
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

        let enabled = world_data::toggle_education_features(&mut data.root_tag);

        world_data::save_server_level_dat(filesystem.as_ref(), &root, &world_path, &data)
            .await
            .map_err(|e| ApiResponse::error(&format!("Failed to save level.dat: {}", e)))?;

        ApiResponse::json(Response {
            message: "Education features toggled".to_string(),
            enabled,
        })
        .ok()
    }
}
