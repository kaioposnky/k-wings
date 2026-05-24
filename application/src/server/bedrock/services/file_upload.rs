use anyhow::{Context, Result, bail};
use reqwest::Client;
use std::path::Path;

use crate::server::bedrock::services::utilities;
use crate::server::filesystem::virtualfs::VirtualWritableFilesystem;

const MAX_RETRIES: u32 = 2;
const MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024; // 1GB
const UPLOAD_TIMEOUT_SECS: u64 = 300;

pub async fn download_file_from_url(url: &str) -> Result<Vec<u8>> {
    let modified_url = url.replace("https://edge.", "https://mediafilez.");

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(UPLOAD_TIMEOUT_SECS))
        .build()
        .context("Failed to create HTTP client")?;

    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=MAX_RETRIES {
        match client.get(&modified_url).send().await {
            Ok(response) => {
                if let Some(content_length) = response.content_length() {
                    if content_length > MAX_FILE_SIZE {
                        bail!("File exceeds 1GB limit");
                    }
                }

                match response.bytes().await {
                    Ok(bytes) => {
                        if bytes.is_empty() {
                            last_error = Some(anyhow::anyhow!("Download failed - empty file"));
                            if attempt < MAX_RETRIES {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                continue;
                            }
                        } else {
                            return Ok(bytes.to_vec());
                        }
                    }
                    Err(e) => {
                        last_error = Some(e.into());
                        if attempt < MAX_RETRIES {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                last_error = Some(e.into());
                if attempt < MAX_RETRIES {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Download failed after all retries")))
}

pub async fn download_and_write_to_server(
    writable_fs: &dyn VirtualWritableFilesystem,
    url: &str,
    dest_path: &Path,
) -> Result<()> {
    let bytes = download_file_from_url(url).await?;
    utilities::write_file(writable_fs, dest_path, &bytes).await
}
