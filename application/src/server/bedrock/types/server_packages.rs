use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::package::Package;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct ServerPackages {
    #[serde(default)]
    pub behaviors: Vec<Package>,
    #[serde(default)]
    pub resources: Vec<Package>,
}
