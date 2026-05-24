use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BedrockVersion {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
}

impl BedrockVersion {
    pub fn from_string(version_str: &str) -> Option<Self> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() < 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }

    pub fn is_version_compatible(server_version: &BedrockVersion, package_version: &BedrockVersion) -> bool {
        server_version.major == package_version.major && server_version.minor == package_version.minor
    }
}
