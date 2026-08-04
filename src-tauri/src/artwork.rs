use crate::{
    settings::DatabaseState,
    steamgriddb::{
        ArtworkFilterKind, ArtworkKind, ArtworkSlot, SteamGridDbError, SteamGridDbRemoteAsset,
    },
};
use image::{
    codecs::webp::{WebPDecoder, WebPEncoder},
    ExtendedColorType, ImageEncoder, ImageFormat, ImageReader,
};
use reqwest::{redirect::Policy, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;

const MAX_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_PIXELS: u64 = 50_000_000;
const MAX_DIMENSION: u32 = 10_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyArtworkRequest {
    pub game_id: String,
    pub slot: ArtworkSlot,
    pub style_filter: ArtworkFilterKind,
    pub candidate_id: String,
}

#[derive(Debug, Clone)]
pub struct PreparedArtwork {
    pub game_id: String,
    pub slot: ArtworkSlot,
    pub kind: ArtworkKind,
    pub external_asset_id: i64,
    pub external_game_id: i64,
    pub grid_style: Option<String>,
    pub width: u32,
    pub height: u32,
    pub source_mime_type: String,
    pub cached_mime_type: String,
    pub cache_key: String,
    pub cached_path: String,
    pub checksum: String,
    pub byte_size: u64,
    pub file_reused: bool,
}

#[derive(Debug, Error)]
pub enum ArtworkDownloadError {
    #[error("candidate is unavailable")]
    Candidate(SteamGridDbError),
    #[error("download request could not be created")]
    RequestSetup,
    #[error("artwork host is not allowed")]
    HostNotAllowed,
    #[error("artwork download is unavailable")]
    Offline,
    #[error("artwork download timed out")]
    Timeout,
    #[error("artwork download returned HTTP status {0}")]
    Http(u16),
    #[error("artwork exceeds the maximum allowed size")]
    TooLarge,
    #[error("artwork content is not a supported image")]
    InvalidImage,
    #[error("animated artwork is not supported yet")]
    AnimatedUnsupported,
    #[error("artwork dimensions are not valid")]
    InvalidDimensions,
    #[error("artwork could not be compressed")]
    Compression,
    #[error("artwork cache could not be written")]
    Storage(#[source] std::io::Error),
}

pub async fn prepare_selected_artwork(
    state: &DatabaseState,
    request: ApplyArtworkRequest,
) -> Result<PreparedArtwork, ArtworkDownloadError> {
    let candidate = state
        .steamgriddb_query_cache
        .lock()
        .map_err(|_| ArtworkDownloadError::Candidate(SteamGridDbError::CandidateExpired))?
        .get(
            &request.candidate_id,
            &request.game_id,
            request.slot,
            request.style_filter,
        )
        .map_err(ArtworkDownloadError::Candidate)?;
    let downloaded = download_original(&candidate).await?;
    let compressed = compress_image(&downloaded.bytes, downloaded.format)?;
    let checksum = sha256_hex(&compressed);
    let cache_root = state.data_directory.cache_directory().join("artwork");
    let relative_path = PathBuf::from("artwork").join(format!("{checksum}.webp"));
    let absolute_path = cache_root.join(format!("{checksum}.webp"));
    let file_reused = write_cache_atomically(&cache_root, &absolute_path, &compressed)?;
    Ok(PreparedArtwork {
        game_id: request.game_id,
        slot: request.slot,
        kind: candidate.kind,
        external_asset_id: candidate.external_asset_id,
        external_game_id: candidate.external_game_id,
        grid_style: candidate.grid_style.map(|style| style.as_str().to_string()),
        width: downloaded.width,
        height: downloaded.height,
        source_mime_type: downloaded.format.to_mime_type().to_string(),
        cached_mime_type: "image/webp".to_string(),
        cache_key: format!("steamgriddb:{checksum}"),
        cached_path: relative_path.to_string_lossy().replace('\\', "/"),
        checksum,
        byte_size: compressed.len() as u64,
        file_reused,
    })
}

pub(crate) fn remove_uncommitted_file(
    state: &DatabaseState,
    artwork: &PreparedArtwork,
) -> Result<(), std::io::Error> {
    if artwork.file_reused {
        return Ok(());
    }
    let path = state
        .data_directory
        .cache_directory()
        .join(&artwork.cached_path);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

struct DownloadedImage {
    bytes: Vec<u8>,
    format: ImageFormat,
    width: u32,
    height: u32,
}

async fn download_original(
    candidate: &SteamGridDbRemoteAsset,
) -> Result<DownloadedImage, ArtworkDownloadError> {
    let source_url =
        Url::parse(&candidate.source_url).map_err(|_| ArtworkDownloadError::HostNotAllowed)?;
    if !is_allowed_artwork_host(&source_url) {
        return Err(ArtworkDownloadError::HostNotAllowed);
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .redirect(Policy::custom(|attempt| {
            if is_allowed_artwork_host(attempt.url()) && attempt.previous().len() < 3 {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .user_agent("LumaDeck/SteamGridDBV1")
        .build()
        .map_err(|_| ArtworkDownloadError::RequestSetup)?;
    let response = client.get(source_url).send().await.map_err(|error| {
        if error.is_timeout() {
            ArtworkDownloadError::Timeout
        } else if error.is_redirect() {
            ArtworkDownloadError::HostNotAllowed
        } else {
            ArtworkDownloadError::Offline
        }
    })?;
    if !is_allowed_artwork_host(response.url()) {
        return Err(ArtworkDownloadError::HostNotAllowed);
    }
    if !response.status().is_success() {
        return Err(ArtworkDownloadError::Http(response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SOURCE_BYTES)
    {
        return Err(ArtworkDownloadError::TooLarge);
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ArtworkDownloadError::Offline)?
    {
        if bytes.len() as u64 + chunk.len() as u64 > MAX_SOURCE_BYTES {
            return Err(ArtworkDownloadError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    validate_image(&bytes)
}

fn validate_image(bytes: &[u8]) -> Result<DownloadedImage, ArtworkDownloadError> {
    let format = image::guess_format(bytes).map_err(|_| ArtworkDownloadError::InvalidImage)?;
    if !matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP
    ) {
        return Err(ArtworkDownloadError::InvalidImage);
    }
    if format == ImageFormat::WebP {
        let decoder = WebPDecoder::new(BufReader::new(Cursor::new(bytes)))
            .map_err(|_| ArtworkDownloadError::InvalidImage)?;
        if decoder.has_animation() {
            return Err(ArtworkDownloadError::AnimatedUnsupported);
        }
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ArtworkDownloadError::InvalidImage)?;
    let dimensions = reader
        .into_dimensions()
        .map_err(|_| ArtworkDownloadError::InvalidImage)?;
    let pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > MAX_DIMENSION
        || dimensions.1 > MAX_DIMENSION
        || pixels > MAX_PIXELS
    {
        return Err(ArtworkDownloadError::InvalidDimensions);
    }
    Ok(DownloadedImage {
        bytes: bytes.to_vec(),
        format,
        width: dimensions.0,
        height: dimensions.1,
    })
}

fn compress_image(bytes: &[u8], format: ImageFormat) -> Result<Vec<u8>, ArtworkDownloadError> {
    let image = ImageReader::with_format(Cursor::new(bytes), format)
        .decode()
        .map_err(|_| ArtworkDownloadError::InvalidImage)?
        .to_rgba8();
    let mut encoded = Vec::new();
    WebPEncoder::new_lossless(&mut encoded)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|_| ArtworkDownloadError::Compression)?;
    Ok(encoded)
}

fn write_cache_atomically(
    cache_root: &Path,
    destination: &Path,
    bytes: &[u8],
) -> Result<bool, ArtworkDownloadError> {
    fs::create_dir_all(cache_root).map_err(ArtworkDownloadError::Storage)?;
    if destination.exists() {
        return Ok(true);
    }
    let temporary = cache_root.join(format!(
        ".{}.tmp-{}",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("artwork"),
        std::process::id()
    ));
    fs::write(&temporary, bytes).map_err(ArtworkDownloadError::Storage)?;
    if let Err(error) = fs::rename(&temporary, destination) {
        let _ = fs::remove_file(&temporary);
        if destination.exists() {
            return Ok(true);
        }
        return Err(ArtworkDownloadError::Storage(error));
    }
    Ok(false)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn is_allowed_artwork_host(url: &Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| host == "steamgriddb.com" || host.ends_with(".steamgriddb.com"))
}

#[cfg(test)]
mod tests {
    use super::{compress_image, is_allowed_artwork_host, validate_image};
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    fn png_with_alpha() -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_fn(2, 2, |x, y| {
            if x == y {
                Rgba([255, 0, 0, 120])
            } else {
                Rgba([0, 0, 255, 255])
            }
        }));
        let mut bytes = Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("png fixture");
        bytes.into_inner()
    }

    #[test]
    fn validates_and_compresses_png_with_alpha_as_webp() {
        let source = png_with_alpha();
        let validated = validate_image(&source).expect("valid image");
        let compressed = compress_image(&validated.bytes, validated.format).expect("webp");
        assert!(compressed.starts_with(b"RIFF"));
        assert_eq!((validated.width, validated.height), (2, 2));
    }

    #[test]
    fn allows_only_steamgriddb_https_hosts() {
        assert!(is_allowed_artwork_host(
            &reqwest::Url::parse("https://images.steamgriddb.com/a.png").expect("url")
        ));
        assert!(!is_allowed_artwork_host(
            &reqwest::Url::parse("https://example.com/a.png").expect("url")
        ));
        assert!(!is_allowed_artwork_host(
            &reqwest::Url::parse("http://images.steamgriddb.com/a.png").expect("url")
        ));
    }
}
