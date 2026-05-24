use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::Path;

use crate::server::Server;
use crate::server::bedrock::services::nbt::{self, NbtValue};
use crate::server::bedrock::types::experiment::Experiment;
use crate::server::filesystem::virtualfs::{VirtualReadableFilesystem, VirtualWritableFilesystem};
use crate::server::bedrock::services::utilities;

/// Bedrock level.dat: 4 bytes header (format version) + 4 bytes tamanho NBT (little-endian u32) + NBT data

#[derive(Debug, Clone)]
pub struct LevelDatData {
    pub format_version: Vec<u8>,
    pub root_tag: HashMap<String, NbtValue>,
}

/// Lê o level.dat binário do Bedrock Edition.
/// Formato: [4 bytes format_version][4 bytes nbt_length LE][nbt_data...]
pub fn read_level_dat(raw_bytes: &[u8]) -> Result<LevelDatData> {
    if raw_bytes.len() < 8 {
        bail!("level.dat too small: {} bytes", raw_bytes.len());
    }

    let format_version = raw_bytes[0..4].to_vec();

    let nbt_length = u32::from_le_bytes(
        raw_bytes[4..8]
            .try_into()
            .context("Failed to read NBT length")?,
    ) as usize;

    if raw_bytes.len() < 8 + nbt_length {
        bail!(
            "level.dat truncated: expected {} bytes of NBT data, got {}",
            nbt_length,
            raw_bytes.len() - 8
        );
    }

    let nbt_data = &raw_bytes[8..8 + nbt_length];

    let root_tag = nbt::parse_root(nbt_data)
        .context("Failed to parse NBT data from level.dat")?;

    Ok(LevelDatData {
        format_version,
        root_tag,
    })
}

/// Serializa o level.dat de volta para bytes binários.
pub fn write_level_dat(data: &LevelDatData) -> Result<Vec<u8>> {
    let nbt_bytes = nbt::serialize_root(&data.root_tag)
        .context("Failed to serialize NBT data")?;

    let nbt_length = nbt_bytes.len() as u32;

    let mut result = Vec::with_capacity(8 + nbt_bytes.len());
    result.extend_from_slice(&data.format_version);
    result.extend_from_slice(&nbt_length.to_le_bytes());
    result.extend_from_slice(&nbt_bytes);

    Ok(result)
}

/// Lê o level.dat do filesystem do servidor e retorna os dados parseados.
pub async fn read_server_level_dat(
    _server: &Server,
    filesystem: &dyn VirtualReadableFilesystem,
    root: &Path,
) -> Result<(String, LevelDatData)> {
    let world_path = utilities::get_default_world_folder(filesystem, root).await?;
    let level_dat_path = root.join(&world_path).join("level.dat");

    let raw_bytes = utilities::read_file_to_bytes(filesystem, &level_dat_path)
        .await
        .context("World not created yet")?;

    let data = read_level_dat(&raw_bytes)?;
    Ok((world_path, data))
}

/// Salva o level.dat modificado de volta no filesystem.
pub async fn save_server_level_dat(
    filesystem: &dyn VirtualWritableFilesystem,
    root: &Path,
    world_path: &str,
    data: &LevelDatData,
) -> Result<()> {
    let level_dat_path = root.join(world_path).join("level.dat");
    let bytes = write_level_dat(data)?;
    utilities::write_file(filesystem, &level_dat_path, &bytes).await
}

/// Extrai os experiments do compound tag "experiments" no level.dat.
pub fn get_experiments(root_tag: &HashMap<String, NbtValue>) -> Vec<Experiment> {
    let mut experiments = Vec::new();

    if let Some(NbtValue::Compound(exp_map)) = root_tag.get("experiments") {
        for (key, val) in exp_map {
            if let NbtValue::Byte(b) = val {
                experiments.push(Experiment {
                    id: key.clone(),
                    enabled: *b as i32,
                });
            }
        }
    }

    experiments
}

/// Atualiza os experiments no root_tag e retorna os que mudaram.
pub fn update_experiments(
    root_tag: &mut HashMap<String, NbtValue>,
    experiments: &[Experiment],
) -> Vec<Experiment> {
    let old_experiments = get_experiments(root_tag);

    let exp_map = root_tag
        .entry("experiments".to_string())
        .or_insert_with(|| NbtValue::Compound(Default::default()));

    if let NbtValue::Compound(exp_compound) = exp_map {
        for exp in experiments {
            exp_compound.insert(exp.id.clone(), NbtValue::Byte(exp.enabled as i8));
        }
    }

    let new_experiments = get_experiments(root_tag);
    get_experiments_diff(&old_experiments, &new_experiments)
}

fn get_experiments_diff(old: &[Experiment], new: &[Experiment]) -> Vec<Experiment> {
    let mut old_map = std::collections::HashMap::new();
    for exp in old {
        old_map.insert(&exp.id, exp.enabled);
    }

    new.iter()
        .filter(|exp| match old_map.get(&exp.id) {
            Some(&old_val) => old_val != exp.enabled,
            None => true,
        })
        .cloned()
        .collect()
}

/// Obtém o InventoryVersion do level.dat.
pub fn get_world_version(root_tag: &HashMap<String, NbtValue>) -> Option<String> {
    if let Some(NbtValue::String(version)) = root_tag.get("InventoryVersion") {
        return Some(version.clone());
    }
    None
}

/// Toggle educationFeaturesEnabled no level.dat. Retorna o novo valor.
pub fn toggle_education_features(root_tag: &mut HashMap<String, NbtValue>) -> bool {
    let current = match root_tag.get("educationFeaturesEnabled") {
        Some(NbtValue::Byte(b)) => *b == 1,
        _ => false,
    };

    let new_value = !current;
    root_tag.insert(
        "educationFeaturesEnabled".to_string(),
        NbtValue::Byte(if new_value { 1 } else { 0 }),
    );

    new_value
}

/// Checa se educationFeaturesEnabled está habilitado.
pub fn get_education_features_enabled(root_tag: &HashMap<String, NbtValue>) -> bool {
    if let Some(NbtValue::Byte(b)) = root_tag.get("educationFeaturesEnabled") {
        return *b == 1;
    }
    false
}
