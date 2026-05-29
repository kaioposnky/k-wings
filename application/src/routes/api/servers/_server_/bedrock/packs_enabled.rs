pub mod get {
    use crate::response::{ApiResponse, ApiResponseResult};
    use crate::routes::api::servers::_server_::GetServer;
    use crate::server::bedrock::services::{utilities, world_packages};
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
        enabled: bool,
    }

    #[utoipa::path(get, path = "/packages/enabled", responses(
        (status = OK, body = inline(Response)),
    ), params(
        ("server" = uuid::Uuid, description = "The server uuid"),
    ))]
    pub async fn route(server: GetServer) -> ApiResponseResult {
        let (root, filesystem) = server
            .filesystem
            .resolve_readable_fs(&server, std::path::Path::new(""))
            .await;

        let world_path = match utilities::get_default_world_folder(filesystem.as_ref(), &root).await
        {
            Ok(p) => p,
            Err(_) => {
                return ApiResponse::new_serialized(Response {
                    message: "Packs not enabled".to_string(),
                    enabled: false,
                })
                .ok();
            }
        };

        let enabled =
            world_packages::server_packs_enabled(filesystem.as_ref(), &root, &world_path).await;

        ApiResponse::new_serialized(Response {
            message: "Check completed".to_string(),
            enabled,
        })
        .ok()
    }
}
