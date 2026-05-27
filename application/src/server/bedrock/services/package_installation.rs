use anyhow::{Context, Result};
use rand::Rng;
use std::path::{Path, PathBuf};

use crate::server::Server;
use crate::server::bedrock::services::utilities::{self, BEDROCK_IGNORED_FOLDERS};
use crate::server::bedrock::types::manifest_info::ManifestInfo;
use crate::server::bedrock::types::package::Package;
use crate::server::filesystem::archive::Archive;
use crate::server::filesystem::virtualfs::{VirtualReadableFilesystem, VirtualWritableFilesystem};

const TEMP_FOLDER: &str = "package-installation";

const PACKAGE_FILE_EXTENSIONS: &[&str] = &[".mcpack", ".mcaddon", ".zip", ".rar"];

pub async fn install_package(
    server: &Server,
    filesystem: &dyn VirtualReadableFilesystem,
    writable_fs: &dyn VirtualWritableFilesystem,
    root: &Path,
    world_path: &str,
    package: &Package,
) -> Result<Vec<ManifestInfo>> {
    let mut manifests = Vec::new();
    let rand_suffix: u64 = rand::rng().random_range(1..99999999);
    let temp_folder_name = format!("{}-{}", TEMP_FOLDER, rand_suffix);
    let temp_path = root.join(&temp_folder_name);

    let result = async {
        writable_fs
            .async_create_dir_all(&temp_path)
            .await
            .context("Failed to create temp directory")?;

        let file_name: String;

        if let Some(ref download_url) = package.download_url {
            let sanitized_name: String = package.name
                .replace('/', "-")
                .replace('\\', "-")
                .chars()
                .filter(|c| !c.is_control())
                .collect();
            file_name = format!("{}.mcaddon", sanitized_name);
            let bytes =
                crate::server::bedrock::services::file_upload::download_file_from_url(download_url)
                    .await?;
            let dest = temp_path.join(&file_name);
            utilities::write_file(writable_fs, &dest, &bytes).await?;
        } else {
            let sanitized_name: String = package.name
                .replace('/', "-")
                .replace('\\', "-")
                .chars()
                .filter(|c| !c.is_control())
                .collect();
            file_name = sanitized_name.clone();
            let src = root.join(&file_name);
            let dst = temp_path.join(&file_name);
            writable_fs
                .async_rename(&src, &dst)
                .await
                .context("Failed to move package file to temp dir")?;
        }

        let archive_path = temp_path.join(&file_name);
        let archive = Archive::open(server.clone(), archive_path.clone())
            .await
            .context("Failed to open archive")?;
        archive
            .extract(temp_path.clone(), None, None)
            .await
            .context("Failed to decompress package")?;

        setup_world_packs(filesystem, writable_fs, root, world_path).await?;

        let mut manifest_paths = Vec::new();
        get_manifests_from_package_files(
            server,
            filesystem,
            writable_fs,
            &file_name,
            &temp_path,
            &mut manifest_paths,
        )
        .await?;

        for (_, manifest_path) in manifest_paths.iter().enumerate() {
            let manifest_file = match utilities::read_file_to_string(filesystem, manifest_path)
                .await
            {
                Ok(content) => {
                    content
                },
                Err(e) => {
                    tracing::warn!("Failed to read manifest on first attempt: {:?}, retrying extraction", e);
                    if let Ok(archive) = Archive::open(server.clone(), archive_path.clone()).await {
                        archive.extract(temp_path.clone(), None, None).await.ok();
                    }
                    utilities::read_file_to_string(filesystem, manifest_path)
                        .await
                        .context("Failed to read manifest after retry")?
                }
            };

            let mut manifest_info = utilities::get_pack_manifest_info(&manifest_file)
                .context("O pacote parece ter um formato ainda não conhecido para nós!")?;

            if manifest_info.pack_type == "behavior" {
                add_pack_to_world_json(
                    filesystem,
                    writable_fs,
                    root,
                    world_path,
                    "world_behavior_packs.json",
                    &manifest_info.uuid,
                    &manifest_info.version,
                )
                .await?;
            } else if manifest_info.pack_type == "resource" {
                add_pack_to_world_json(
                    filesystem,
                    writable_fs,
                    root,
                    world_path,
                    "world_resource_packs.json",
                    &manifest_info.uuid,
                    &manifest_info.version,
                )
                .await?;
            }

            let mut origin_dir = manifest_path
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf();

            let origin_entries = filesystem
                .async_read_dir(&origin_dir, None, 1, Default::default())
                .await
                .map(|l| l.entries)
                .unwrap_or_default();
            if origin_entries.len() == 1 && origin_entries[0].directory {
                origin_dir = origin_dir.join(origin_entries[0].name.as_str());
            }

            let mut package_folder_name = origin_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            if origin_dir == temp_path {
                let clean_name: String = manifest_info
                    .name
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                package_folder_name = clean_name;
            }

            let version_str = manifest_info
                .version
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(".");

            let dest_dir = root
                .join(world_path)
                .join(format!("{}_packs", manifest_info.pack_type))
                .join(format!(
                    "{}-{}-v{}",
                    package_folder_name, manifest_info.pack_type, version_str
                ));


            let _ = writable_fs.async_remove_dir_all(&dest_dir).await;

            writable_fs
                .async_rename(&origin_dir, &dest_dir)
                .await
                .context("Failed to move package to destination")?;

            manifest_info.folder_path = dest_dir
                .strip_prefix(root)
                .unwrap_or(&dest_dir)
                .to_string_lossy()
                .to_string();
            manifests.push(manifest_info);
        }

        Ok::<Vec<ManifestInfo>, anyhow::Error>(manifests)
    }
    .await;

    let _ = writable_fs.async_remove_dir_all(&temp_path).await;

    match &result {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Installation FAILED: {}", e);
            tracing::error!("=== INSTALL_PACKAGE END (ERROR) ===");
        }
    }

    result
}

async fn get_manifests_from_package_files(
    server: &Server,
    filesystem: &dyn VirtualReadableFilesystem,
    writable_fs: &dyn VirtualWritableFilesystem,
    package_file_name: &str,
    path: &Path,
    manifest_paths: &mut Vec<PathBuf>,
) -> Result<()> {
    let listing = filesystem
        .async_read_dir(&path, None, 1, Default::default())
        .await
        .context("Failed to list package directory")?;

    let entries: Vec<_> = listing
        .entries
        .iter()
        .filter(|f| {
            if !f.directory {
                return true;
            }
            !BEDROCK_IGNORED_FOLDERS.contains(&f.name.to_lowercase().as_str())
        })
        .collect();

    for entry in &entries {
        let name = entry.name.as_str();
        if name == package_file_name {
            continue;
        }

        let file_path = path.join(name);

        if name.contains("manifest.json") {
            if !manifest_paths.contains(&file_path) {
                manifest_paths.push(file_path);
            }
            return Ok(());
        }

        if entry.directory {
            Box::pin(get_manifests_from_package_files(
                server,
                filesystem,
                writable_fs,
                package_file_name,
                &file_path,
                manifest_paths,
            ))
            .await?;
        } else if str_ends_with_multiple(name, PACKAGE_FILE_EXTENSIONS) {
            let mut base_folder_name = name.to_string();
            for ext in PACKAGE_FILE_EXTENSIONS {
                if base_folder_name.ends_with(ext) {
                    base_folder_name =
                        base_folder_name[..base_folder_name.len() - ext.len()].to_string();
                    break;
                }
            }

            let clean_folder_name: String = base_folder_name
                .replace(' ', "_")
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
                .collect();

            let clean_file_name: String = name
                .replace(' ', "_")
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
                .collect();


            if entry.mime == "application/zip"
                || str_ends_with_multiple(name, &[".mcpack", ".mcaddon", ".zip"])
            {
                let new_folder_path = if path.file_name().map(|f| f.to_string_lossy().to_string())
                    == Some(clean_folder_name.clone())
                {
                    path.to_path_buf()
                } else {
                    let folder = path.join(&clean_folder_name);
                    writable_fs.async_create_dir_all(&folder).await.ok();
                    folder
                };

                let new_file_path = new_folder_path.join(&clean_file_name);

                writable_fs
                    .async_rename(&file_path, &new_file_path)
                    .await
                    .ok();

                if let Ok(archive) = Archive::open(server.clone(), new_file_path.clone()).await {
                    archive
                        .extract(new_folder_path.clone(), None, None)
                        .await
                        .ok();
                }
                let _ = writable_fs.async_remove_file(&new_file_path).await;

                Box::pin(get_manifests_from_package_files(
                    server,
                    filesystem,
                    writable_fs,
                    package_file_name,
                    &new_folder_path,
                    manifest_paths,
                ))
                .await?;
            }
        }
    }

    Ok(())
}

fn str_ends_with_multiple(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.ends_with(n))
}

async fn setup_world_packs(
    filesystem: &dyn VirtualReadableFilesystem,
    writable_fs: &dyn VirtualWritableFilesystem,
    root: &Path,
    world_path: &str,
) -> Result<()> {
    let world_dir = root.join(world_path);

    writable_fs
        .async_create_dir_all(&world_dir.join("behavior_packs"))
        .await
        .ok();
    writable_fs
        .async_create_dir_all(&world_dir.join("resource_packs"))
        .await
        .ok();

    let bp_json = world_dir.join("world_behavior_packs.json");
    let rp_json = world_dir.join("world_resource_packs.json");

    if utilities::read_file_to_string(filesystem, &bp_json)
        .await
        .is_err()
    {
        utilities::write_file(writable_fs, &bp_json, b"[]")
            .await
            .ok();
    }
    if utilities::read_file_to_string(filesystem, &rp_json)
        .await
        .is_err()
    {
        utilities::write_file(writable_fs, &rp_json, b"[]")
            .await
            .ok();
    }

    Ok(())
}

async fn add_pack_to_world_json(
    filesystem: &dyn VirtualReadableFilesystem,
    writable_fs: &dyn VirtualWritableFilesystem,
    root: &Path,
    world_path: &str,
    json_filename: &str,
    uuid: &str,
    version: &[i32],
) -> Result<()> {
    let path = root.join(world_path).join(json_filename);
    let content = utilities::read_file_to_string(filesystem, &path)
        .await
        .unwrap_or_else(|_| {
            "[]".to_string()
        });

    #[derive(serde::Serialize, serde::Deserialize)]
    struct PackInfo {
        pack_id: String,
        version: Vec<i32>,
    }

    let mut pack_list: Vec<PackInfo> = serde_json::from_str(&content).unwrap_or_default();

    let new_entry = PackInfo {
        pack_id: uuid.to_string(),
        version: version.to_vec(),
    };

    if let Some(idx) = pack_list.iter().position(|p| p.pack_id == uuid) {
        pack_list[idx] = new_entry;
    } else {
        pack_list.push(new_entry);
    }

    let raw = serde_json::to_string_pretty(&pack_list)?;
    utilities::write_file(writable_fs, &path, raw.as_bytes()).await?;

    Ok(())
}
