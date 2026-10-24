//! Manifest-driven, explicit model download. Runtime inference never downloads.

use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, Deserialize)]
struct Manifest {
    model: Vec<Model>,
}

#[derive(Debug, Deserialize)]
struct Model {
    id: String,
    repo: String,
    #[serde(default)]
    revision: String,
    file: String,
    sha256: String,
    license: String,
}

pub fn run(args: impl IntoIterator<Item = String>) -> Result<()> {
    let mut id = None;
    let mut allow_unverified = false;
    for argument in args {
        match argument.as_str() {
            "--allow-unverified" => allow_unverified = true,
            _ if argument.starts_with('-') => {
                return Err(format!("unknown option: {argument}").into());
            }
            _ if id.is_none() => id = Some(argument),
            _ => return Err("usage: cargo xtask fetch-model <id> [--allow-unverified]".into()),
        }
    }
    let id = id.ok_or("usage: cargo xtask fetch-model <id> [--allow-unverified]")?;
    let manifest: Manifest = toml::from_str(include_str!("../../models/manifest.toml"))?;
    let model = manifest
        .model
        .into_iter()
        .find(|model| model.id == id)
        .ok_or_else(|| format!("unknown model: {id}"))?;
    if Path::new(&model.file)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("gguf"))
        || Path::new(&model.file)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(&model.file)
        || model.file.contains(['/', '\\'])
    {
        return Err("manifest does not specify a downloadable GGUF filename".into());
    }
    if model.revision.len() != 40 || !model.revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("manifest must pin the model revision to a full repository commit SHA".into());
    }
    let has_hash =
        model.sha256.len() == 64 && model.sha256.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !has_hash {
        if model.sha256 != "TBD" || !allow_unverified {
            return Err("manifest has no valid SHA-256; TBD requires --allow-unverified".into());
        }
        // Warn before either the existing-file path or a new download.
        eprintln!(
            "Warning: manifest SHA-256 is TBD; --allow-unverified permits an unverified download or reuse."
        );
    }
    println!("{}: licence {}", model.id, model.license);
    let directory =
        models_directory().ok_or("cannot locate models directory; set OFFHAND_MODELS_DIR")?;
    fs::create_dir_all(&directory)?;
    let destination = directory.join(&model.file);
    if destination.is_file() {
        let hash = hash_file(&destination)?;
        if has_hash && !hash.eq_ignore_ascii_case(&model.sha256) {
            return Err(format!(
                "existing model checksum mismatch: {} (SHA-256 {hash}); remove it before retrying",
                destination.display()
            )
            .into());
        }
        println!("{}\nSHA-256: {hash}", destination.display());
        return Ok(());
    }
    let temporary = directory.join(format!("{}.{}.part", model.file, std::process::id()));
    let result = download(&model, &temporary, &destination, has_hash);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn models_directory() -> Option<PathBuf> {
    std::env::var_os("OFFHAND_MODELS_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|path| path.join("offhand").join("models")))
}

fn download(model: &Model, temporary: &Path, destination: &Path, has_hash: bool) -> Result<()> {
    let url = format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        model.repo, model.revision, model.file
    );
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(1800))
        .build()?;
    println!("Downloading {}", model.id);
    let mut response = client.get(url).send()?.error_for_status()?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    let mut digest = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = response.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
        bytes += count as u64;
    }
    let hash = format!("{:x}", digest.finalize());
    println!("SHA-256: {hash}");
    if bytes == 0 {
        return Err("download was empty".into());
    }
    if has_hash && !hash.eq_ignore_ascii_case(&model.sha256) {
        return Err("download checksum does not match manifest".into());
    }
    output.sync_all()?;
    drop(output);
    fs::rename(temporary, destination)?;
    println!("Saved {bytes} bytes to {}", destination.display());
    Ok(())
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
