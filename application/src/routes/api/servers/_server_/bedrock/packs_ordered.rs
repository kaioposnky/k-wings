pub mod get {
    use crate::response::{ApiResponse, ApiResponseResult};
    use crate::routes::api::servers::_server_::GetServer;
    use crate::server::bedrock::services::{utilities, world_packages};
    use crate::server::bedrock::types::server_packages::ServerPackages;
    use axum::http::StatusCode;
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
        data: ServerPackages,
    }

    #[utoipa::path(get, path = "/packages/ordered", responses(
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

        let world_path = utilities::get_default_world_folder(filesystem.as_ref(), &root)
            .await
            .map_err(|_| {
                ApiResponse::error("World not created yet").with_status(StatusCode::BAD_REQUEST)
            })?;

        let result =
            world_packages::get_server_packs_ordered(filesystem.as_ref(), &root, &world_path)
                .await
                .map_err(|e| ApiResponse::error(&format!("Failed to get ordered packs: {}", e)))?;

        ApiResponse::json(Response {
            message: "Ordered packs retrieved successfully".to_string(),
            data: result,
        })
        .ok()
    }
}
