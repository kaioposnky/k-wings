pub mod post {
    use crate::response::{ApiResponse, ApiResponseResult};
    use crate::routes::api::servers::_server_::GetServer;
    use crate::server::bedrock::services::{utilities, world_packages};
    use axum::Json;
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct UpdatePackOrderRequest {
        pack_type: String,
        source_uuid: String,
        destination_position: usize,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: String,
    }

    #[utoipa::path(post, path = "/packages/reorder", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = crate::routes::ApiError),
    ), params(
        ("server" = uuid::Uuid, description = "The server uuid"),
    ), request_body = UpdatePackOrderRequest)]
    pub async fn route(
        server: GetServer,
        Json(body): Json<UpdatePackOrderRequest>,
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

        world_packages::update_server_pack_order(
            filesystem.as_ref(),
            filesystem.as_ref(),
            &root,
            &world_path,
            &body.pack_type,
            &body.source_uuid,
            body.destination_position,
        )
        .await
        .map_err(|e| ApiResponse::error(&format!("{}", e)).with_status(StatusCode::BAD_REQUEST))?;

        ApiResponse::json(Response {
            message: "Pack order updated successfully".to_string(),
        })
        .ok()
    }
}
