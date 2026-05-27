use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;
use tokio::io::AsyncReadExt;

use crate::server::bedrock::types::manifest_info::ManifestInfo;
use crate::server::filesystem::virtualfs::VirtualReadableFilesystem;

pub const BEDROCK_IGNORED_FOLDERS: &[&str] = &[
    "animations",
    "animation_controllers",
    "biomes",
    "blocks",
    "items",
    "item_catalog",
    "scripts",
    "structures",
    "subpacks",
    "texts",
    "textures",
    "ui",
    "trading",
    "fogs",
    "font",
    "sounds",
    "entities",
    "features",
    "feature_rules",
    "functions",
    "loot_tables",
    "recipes",
    "spawn_rules",
    "worldgen",
    "cameras",
    "documentation",
    "dialogue",
    "jigsaw_pools",
    "exp",
    "attachables",
    "entity",
    "materials",
    "models",
    "particles",
    "render_controllers",
    "uid",
    "cooldowns",
    "input_methods",
    "layouts",
    "atmospherics",
    "color_grading",
    "lighting",
    "pbr",
    "shadows",
    "water",
    "editor",
    "db",
    "marketing",
    "premium_cache",
    "persona",
    "skins",
    "pokeitems",
    "__macosx",
    ".git",
    ".github",
    ".vscode",
    ".idea",
];

/// Lê server.properties e extrai o level-name, retornando "worlds/{level-name}".
pub async fn get_default_world_folder(
    filesystem: &dyn VirtualReadableFilesystem,
    root: &Path,
) -> Result<String> {
    let folder_name = get_default_world_folder_from_properties(filesystem, root).await?;
    Ok(format!("worlds/{}", folder_name))
}

async fn get_default_world_folder_from_properties(
    filesystem: &dyn VirtualReadableFilesystem,
    root: &Path,
) -> Result<String> {
    let props_path = root.join("server.properties");
    let file_read = filesystem
        .async_read_file(&props_path, None)
        .await
        .context("Failed to read server.properties")?;

    let mut content = String::new();
    tokio::io::BufReader::new(file_read.reader)
        .read_to_string(&mut content)
        .await
        .context("Failed to read server.properties content")?;

    let re = Regex::new(r"(?m)^level-name\s*=\s*(.*)$").unwrap();
    if let Some(caps) = re.captures(&content) {
        if let Some(m) = caps.get(1) {
            let name = m.as_str().trim();
            if !name.is_empty() {
                // Remove barras invertidas do nome do mundo
                let clean_name = name.replace('\\', "");
                return Ok(clean_name);
            }
        }
    }

    Ok("Bedrock level".to_string())
}

/// Remove BOM UTF-8, comentários (/* */ e //), trailing commas, e corrige JSON malformado.
pub fn clean_json_content(raw: &str) -> Result<serde_json::Value> {
    let mut content = raw.to_string();

    // Remove BOM UTF-8
    if content.starts_with('\u{FEFF}') {
        content = content[3..].to_string();
    } else if content.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
        content = content[3..].to_string();
    }

    // Remove block comments /* ... */
    let block_comment_re = Regex::new(r"(?s)/\*.*?\*/").unwrap();
    content = block_comment_re.replace_all(&content, "").to_string();

    // Remove line comments // (mas não dentro de strings)
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let url_in_string_re = Regex::new(r#""([^"]*)//(.*)""#).unwrap();
    for line in &mut lines {
        if line.contains("//") && !url_in_string_re.is_match(line) {
            if let Some(pos) = line.find("//") {
                line.truncate(pos);
            }
        }
    }
    content = lines.join("\n");

    // Remove trailing commas antes de } ou ]
    let trailing_comma_re = Regex::new(r",\s*([}\]])").unwrap();
    content = trailing_comma_re.replace_all(&content, "$1").to_string();

    match serde_json::from_str(&content) {
        Ok(val) => Ok(val),
        Err(_) => {
            let fixed = fix_unbalanced_json_content(&content);
            serde_json::from_str(&fixed)
                .context("Failed to parse manifest JSON even after fix attempt")
        }
    }
}

fn fix_unbalanced_json_content(content: &str) -> String {
    let mut escaped = false;
    let mut in_string = false;
    let mut stack: Vec<char> = Vec::new();
    let mut result = String::with_capacity(content.len());
    let chars: Vec<char> = content.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];

        if escaped {
            escaped = false;
            result.push(ch);
            i += 1;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            result.push(ch);
            i += 1;
            continue;
        }

        if ch == '"' {
            in_string = !in_string;
            result.push(ch);
            i += 1;
            continue;
        }

        if !in_string && ch == '0' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            i += 1;
            continue;
        }

        if in_string {
            result.push(ch);
            i += 1;
            continue;
        }

        match ch {
            '{' => {
                stack.push('}');
                result.push(ch);
            }
            '[' => {
                stack.push(']');
                result.push(ch);
            }
            '}' | ']' => {
                if let Some(&last) = stack.last() {
                    if last == ch {
                        stack.pop();
                        result.push(ch);
                    }
                }
            }
            _ => {
                result.push(ch);
            }
        }

        i += 1;
    }

    if in_string {
        result.push('"');
    }

    while let Some(closer) = stack.pop() {
        result.push(closer);
    }

    result
}

/// Busca uma chave recursivamente num serde_json::Value
pub fn find_key_recursively<'a>(
    key: &str,
    data: &'a serde_json::Value,
) -> Option<&'a serde_json::Value> {
    if let Some(obj) = data.as_object() {
        if let Some(val) = obj.get(key) {
            if val.is_array() {
                return Some(val);
            }
        }
        for (_, v) in obj {
            if let Some(found) = find_key_recursively(key, v) {
                return Some(found);
            }
        }
    }
    if let Some(arr) = data.as_array() {
        for v in arr {
            if let Some(found) = find_key_recursively(key, v) {
                return Some(found);
            }
        }
    }
    None
}

fn parse_version(version: &serde_json::Value) -> Vec<i32> {
    if let Some(arr) = version.as_array() {
        return arr
            .iter()
            .filter_map(|v| v.as_i64().map(|n| n as i32))
            .collect();
    }
    if let Some(s) = version.as_str() {
        return s.split('.').filter_map(|p| p.parse::<i32>().ok()).collect();
    }
    if let Some(n) = version.as_i64() {
        return vec![n as i32];
    }
    vec![]
}

/// Extrai informações do manifest.json de um pacote Bedrock.
pub fn get_pack_manifest_info(json_raw: &str) -> Result<ManifestInfo> {
    let clean_json = clean_json_content(json_raw)?;

    let modules = find_key_recursively("modules", &clean_json);

    let mut pack_type = String::new();
    if let Some(modules_val) = modules {
        if let Some(arr) = modules_val.as_array() {
            for module in arr {
                let module_type = module.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if module_type == "data" || module_type == "script" {
                    pack_type = "behavior".to_string();
                    break;
                }
                if module_type == "resources" {
                    pack_type = "resource".to_string();
                    break;
                }
            }
        }
    }

    let header = clean_json
        .get("header")
        .cloned()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let uuid = header
        .get("uuid")
        .or_else(|| header.get("pack_id"))
        .or_else(|| header.get("identifier"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let version_val = header
        .get("version")
        .or_else(|| header.get("pack_version"))
        .or_else(|| header.get("packs_version"))
        .or_else(|| header.get("pack_versions"))
        .cloned()
        .unwrap_or(serde_json::Value::Array(vec![]));
    let version = parse_version(&version_val);

    let name = header
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let description = header
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(ManifestInfo {
        name,
        description,
        uuid,
        pack_type,
        version,
        folder_path: String::new(),
    })
}

/// Lê um arquivo do filesystem e retorna como String.
pub async fn read_file_to_string(
    filesystem: &dyn VirtualReadableFilesystem,
    path: &Path,
) -> Result<String> {
    let file_read = filesystem
        .async_read_file(&path, None)
        .await
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let mut content = String::new();
    tokio::io::BufReader::new(file_read.reader)
        .read_to_string(&mut content)
        .await?;

    Ok(content)
}

/// Lê um arquivo do filesystem e retorna como bytes.
pub async fn read_file_to_bytes(
    filesystem: &dyn VirtualReadableFilesystem,
    path: &Path,
) -> Result<Vec<u8>> {
    let file_read = filesystem
        .async_read_file(&path, None)
        .await
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let mut content = Vec::new();
    tokio::io::BufReader::new(file_read.reader)
        .read_to_end(&mut content)
        .await?;

    Ok(content)
}

/// Escreve conteúdo em um arquivo no filesystem.
pub async fn write_file(
    filesystem: &dyn crate::server::filesystem::virtualfs::VirtualWritableFilesystem,
    path: &Path,
    content: &[u8],
) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let mut file = filesystem
        .async_create_file(&path)
        .await
        .with_context(|| format!("Failed to create file: {}", path.display()))?;

    file.write_all(content).await?;
    file.shutdown().await?;
    filesystem.async_chown(&path).await?;

    Ok(())
}
