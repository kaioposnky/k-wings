use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestInfo {
    pub name: String,
    pub description: String,
    pub uuid: String,
    pub version: Vec<i32>,
    pub pack_type: String,
    pub folder_path: String,
}
