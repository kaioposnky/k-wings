pub mod post {
    use crate::response::{ApiResponse, ApiResponseResult};
    use crate::routes::api::servers::_server_::GetServer;
    use crate::server::bedrock::services::{package_installation, utilities};
    use crate::server::bedrock::types::manifest_info::ManifestInfo;
    use crate::server::bedrock::types::package::Package;
    use axum::Json;
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct InstallPackageRequest {
        name: String,
        #[serde(default)]
        download_url: Option<String>,
        #[serde(default)]
        #[allow(dead_code)]
        file_names: Option<Vec<String>>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
        manifests: Vec<ManifestInfo>,
    }

    #[utoipa::path(post, path = "/packages/install", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = crate::routes::ApiError),
        (status = INTERNAL_SERVER_ERROR, body = crate::routes::ApiError),
    ), params(
        ("server" = uuid::Uuid, description = "The server uuid"),
    ), request_body = InstallPackageRequest)]
    pub async fn route(
        server: GetServer,
        Json(body): Json<InstallPackageRequest>,
    ) -> ApiResponseResult {
        let (root, filesystem) = server
            .filesystem
            .resolve_writable_fs(&server, std::path::Path::new(""))
            .await;

        let world_path = utilities::get_default_world_folder(filesystem.as_ref(), &root)
            .await
            .map_err(|_| {
                ApiResponse::error("World not created yet — start the server first")
                    .with_status(StatusCode::BAD_REQUEST)
            })?;

        let package = Package {
            name: body.name,
            description: String::new(),
            pack_type: None,
            uuid: None,
            version: None,
            folder_path: None,
            download_url: body.download_url,
            curse_forge_id: None,
            version_id: None,
            website_url: None,
            thumbnail_url: None,
        };

        let manifests = package_installation::install_package(
            &server,
            filesystem.as_ref(),
            filesystem.as_ref(),
            &root,
            &world_path,
            &package,
        )
        .await
        .map_err(|e| {
            ApiResponse::error(&format!("Installation failed: {}", e))
                .with_status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;

        ApiResponse::json(Response {
            message: "Package installed successfully".to_string(),
            manifests,
        })
        .ok()
    }
}
