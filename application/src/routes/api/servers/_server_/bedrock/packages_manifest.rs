pub mod get {
    use crate::response::{ApiResponse, ApiResponseResult};
    use crate::routes::api::servers::_server_::GetServer;
    use crate::server::bedrock::services::{utilities, world_packages};
    use crate::server::bedrock::types::manifest_info::ManifestInfo;
    use axum::http::StatusCode;
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
        behaviors: Vec<ManifestInfo>,
        resources: Vec<ManifestInfo>,
    }

    #[utoipa::path(get, path = "/packages/manifest", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = crate::routes::ApiError),
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
            world_packages::get_server_packages_manifest(filesystem.as_ref(), &root, &world_path)
                .await;

        match result {
            Some(manifest) => ApiResponse::new_serialized(Response {
                message: "Packages manifest retrieved successfully".to_string(),
                behaviors: manifest.behaviors,
                resources: manifest.resources,
            })
            .ok(),
            None => ApiResponse::new_serialized(Response {
                message: "No packages found".to_string(),
                behaviors: vec![],
                resources: vec![],
            })
            .ok(),
        }
    }
}
