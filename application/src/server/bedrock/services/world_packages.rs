use anyhow::{Context, Result, bail};
use std::path::Path;

use crate::server::bedrock::services::utilities;
use crate::server::bedrock::types::manifest_info::ManifestInfo;
use crate::server::bedrock::types::package::Package;
use crate::server::bedrock::types::server_packages::ServerPackages;
use crate::server::filesystem::virtualfs::{VirtualReadableFilesystem, VirtualWritableFilesystem};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PackEntry {
    pub pack_id: String,
    pub version: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct PackagesManifestResult {
    pub behaviors: Vec<ManifestInfo>,
    pub resources: Vec<ManifestInfo>,
}

pub async fn get_server_packages_manifest(
    filesystem: &dyn VirtualReadableFilesystem,
    root: &Path,
    world_path: &str,
) -> Option<PackagesManifestResult> {
    let behaviors = get_server_pack_manifests(filesystem, root, world_path, "behavior_packs")
        .await
        .unwrap_or_default();

    let resources = get_server_pack_manifests(filesystem, root, world_path, "resource_packs")
        .await
        .unwrap_or_default();

    Some(PackagesManifestResult {
        behaviors,
        resources,
    })
}

pub async fn get_server_packs_ordered(
    filesystem: &dyn VirtualReadableFilesystem,
    root: &Path,
    world_path: &str,
) -> Result<ServerPackages> {
    let bp_path = root.join(world_path).join("world_behavior_packs.json");
    let rp_path = root.join(world_path).join("world_resource_packs.json");

    let behavior_packs = read_and_parse_pack_list(filesystem, &bp_path)
        .await
        .unwrap_or_default();
    let resource_packs = read_and_parse_pack_list(filesystem, &rp_path)
        .await
        .unwrap_or_default();

    let behaviors = behavior_packs
        .iter()
        .map(|p| Package {
            name: String::new(),
            description: String::new(),
            pack_type: Some("behavior".to_string()),
            uuid: Some(p.pack_id.clone()),
            version: None,
            folder_path: None,
            download_url: None,
            curse_forge_id: None,
            version_id: None,
            website_url: None,
            thumbnail_url: None,
        })
        .collect();

    let resources = resource_packs
        .iter()
        .map(|p| Package {
            name: String::new(),
            description: String::new(),
            pack_type: Some("resource".to_string()),
            uuid: Some(p.pack_id.clone()),
            version: None,
            folder_path: None,
            download_url: None,
            curse_forge_id: None,
            version_id: None,
            website_url: None,
            thumbnail_url: None,
        })
        .collect();

    Ok(ServerPackages {
        behaviors,
        resources,
    })
}

pub async fn update_server_pack_order(
    filesystem: &dyn VirtualReadableFilesystem,
    writable_fs: &dyn VirtualWritableFilesystem,
    root: &Path,
    world_path: &str,
    pack_type: &str,
    src_uuid: &str,
    dst_idx: usize,
) -> Result<()> {
    let json_filename = match pack_type {
        "behavior" => "world_behavior_packs.json",
        "resource" => "world_resource_packs.json",
        _ => bail!("packType must be \"behavior\" or \"resource\"."),
    };

    let json_path = root.join(world_path).join(json_filename);

    let content = utilities::read_file_to_string(filesystem, &json_path)
        .await
        .context("Failed to read pack config JSON")?;

    let mut packs_list: Vec<PackEntry> = serde_json::from_str(&content).unwrap_or_default();

    let src_idx = packs_list
        .iter()
        .position(|p| p.pack_id == src_uuid)
        .ok_or_else(|| anyhow::anyhow!("Source UUID ({}) not found in JSON.", src_uuid))?;

    if dst_idx >= packs_list.len() {
        bail!("Destination index ({}) out of bounds.", dst_idx);
    }

    let pack_item = packs_list.remove(src_idx);
    packs_list.insert(dst_idx, pack_item);

    let packs_raw = serde_json::to_string_pretty(&packs_list)?;
    utilities::write_file(writable_fs, &json_path, packs_raw.as_bytes()).await?;

    Ok(())
}

pub async fn delete_server_pack(
    filesystem: &dyn VirtualReadableFilesystem,
    writable_fs: &dyn VirtualWritableFilesystem,
    root: &Path,
    world_path: &str,
    package: &Package,
) -> Result<()> {
    let pack_type = package.pack_type.as_deref().unwrap_or("");
    let json_filename = match pack_type {
        "behavior" | "script" => "world_behavior_packs.json",
        "resource" => "world_resource_packs.json",
        _ => bail!("O pacote selecionado está corrompido!"),
    };

    let global_config_path = root.join(world_path).join(json_filename);

    if let Some(ref folder_path) = package.folder_path {
        let abs_path = root.join(folder_path);
        writable_fs
            .async_remove_dir_all(&abs_path)
            .await
            .context("Arquivos do pacote não encontrados!")?;
    }

    let content = utilities::read_file_to_string(filesystem, &global_config_path)
        .await
        .unwrap_or_else(|_| "[]".to_string());
    let mut packs_list: Vec<PackEntry> = serde_json::from_str(&content).unwrap_or_default();

    let pack_uuid = package.uuid.as_deref().unwrap_or("");
    let start_size = packs_list.len();

    packs_list.retain(|p| p.pack_id != pack_uuid);

    if packs_list.len() == start_size {
        bail!("O pacote não existe no servidor ou os arquivos estão corrompidos!");
    }

    let packs_raw = serde_json::to_string_pretty(&packs_list)?;
    utilities::write_file(writable_fs, &global_config_path, packs_raw.as_bytes()).await?;

    Ok(())
}

pub async fn server_packs_enabled(
    filesystem: &dyn VirtualReadableFilesystem,
    root: &Path,
    world_path: &str,
) -> bool {
    let world_dir = root.join(world_path);

    if utilities::read_file_to_string(filesystem, &world_dir.join("world_behavior_packs.json"))
        .await
        .is_err()
    {
        return false;
    }
    if utilities::read_file_to_string(filesystem, &world_dir.join("world_resource_packs.json"))
        .await
        .is_err()
    {
        return false;
    }

    let mut has_any_pack = false;

    if filesystem
        .async_metadata(&world_dir.join("resource_packs"))
        .await
        .is_ok()
    {
        has_any_pack = true;
    }
    if filesystem
        .async_metadata(&world_dir.join("behavior_packs"))
        .await
        .is_ok()
    {
        has_any_pack = true;
    }

    has_any_pack
}

async fn get_server_pack_manifests(
    filesystem: &dyn VirtualReadableFilesystem,
    root: &Path,
    world_path: &str,
    pack_type: &str,
) -> Result<Vec<ManifestInfo>> {
    let pack_type_path = root.join(world_path).join(pack_type);

    let listing = filesystem
        .async_read_dir(&pack_type_path, None, 1, Default::default())
        .await
        .context("Failed to list pack directory")?;

    let mut package_manifests = Vec::new();

    for entry in &listing.entries {
        if entry.directory {
            let folder_path = pack_type_path.join(entry.name.as_str());

            let folder_listing = match filesystem
                .async_read_dir(&folder_path, None, 1, Default::default())
                .await
            {
                Ok(l) => l,
                Err(_) => continue,
            };

            if let Some(mut manifest) =
                get_manifest_info(filesystem, &folder_path, &folder_listing.entries).await
            {
                manifest.folder_path = format!("{}/{}/{}", world_path, pack_type, entry.name);
                package_manifests.push(manifest);
            }
        }
    }

    Ok(package_manifests)
}

async fn get_manifest_info(
    filesystem: &dyn VirtualReadableFilesystem,
    folder_path: &Path,
    entries: &[crate::models::DirectoryEntry],
) -> Option<ManifestInfo> {
    for entry in entries {
        if entry.name.as_str() == "manifest.json" {
            let manifest_path = folder_path.join("manifest.json");
            if let Ok(content) = utilities::read_file_to_string(filesystem, &manifest_path).await {
                if let Ok(mut info) = utilities::get_pack_manifest_info(&content) {
                    info.folder_path = folder_path.to_string_lossy().to_string();
                    return Some(info);
                }
            }
            return None;
        }
    }
    None
}

async fn read_and_parse_pack_list(
    filesystem: &dyn VirtualReadableFilesystem,
    path: &Path,
) -> Result<Vec<PackEntry>> {
    let content = utilities::read_file_to_string(filesystem, path).await?;
    let packs: Vec<PackEntry> = serde_json::from_str(&content).unwrap_or_default();
    Ok(packs)
}
