pub mod post {
    use crate::response::{ApiResponse, ApiResponseResult};
    use crate::routes::api::servers::_server_::GetServer;
    use crate::server::bedrock::services::{utilities, world_packages};
    use crate::server::bedrock::types::package::Package;
    use axum::Json;
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DeletePackRequest {
        pack_type: String,
        pack_uuid: String,
        #[serde(default)]
        folder_path: Option<String>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
    }

    #[utoipa::path(post, path = "/packages/delete", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = crate::routes::ApiError),
        (status = NOT_FOUND, body = crate::routes::ApiError),
    ), params(
        ("server" = uuid::Uuid, description = "The server uuid"),
    ), request_body = DeletePackRequest)]
    pub async fn route(
        server: GetServer,
        Json(body): Json<DeletePackRequest>,
    ) -> ApiResponseResult {
        let (root, filesystem) = server
            .filesystem
            .resolve_writable_fs(&server, std::path::Path::new(""))
            .await;

        let world_path = utilities::get_default_world_folder(filesystem.as_ref(), &root)
            .await
            .map_err(|_| {
                ApiResponse::error("World not created yet").with_status(StatusCode::BAD_REQUEST)
            })?;

        let package = Package {
            name: String::new(),
            description: String::new(),
            pack_type: Some(body.pack_type),
            uuid: Some(body.pack_uuid),
            version: None,
            folder_path: body.folder_path,
            download_url: None,
            curse_forge_id: None,
            version_id: None,
            website_url: None,
            thumbnail_url: None,
        };

        world_packages::delete_server_pack(
            filesystem.as_ref(),
            filesystem.as_ref(),
            &root,
            &world_path,
            &package,
        )
        .await
        .map_err(|e| ApiResponse::error(&format!("{}", e)).with_status(StatusCode::BAD_REQUEST))?;

        ApiResponse::new_serialized(Response {
            message: "Package deleted successfully".to_string(),
        })
        .ok()
    }
}
