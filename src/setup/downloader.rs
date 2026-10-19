use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tar::Archive;
use tracing::{info, warn};

use crate::config;

static MAX_DOWNLOAD_BYTES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(256 << 20);

pub fn max_download_bytes() -> u64 {
    MAX_DOWNLOAD_BYTES.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn set_max_download_bytes(v: u64) {
    MAX_DOWNLOAD_BYTES.store(v, std::sync::atomic::Ordering::Relaxed);
}

pub fn download_file(url: &str, dest: &Path) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let mut resp = client.get(url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("bad status: {}", resp.status());
    }

    let tmp_dest = dest.with_extension("tmp");
    let mut out = create_new_file(&tmp_dest)?;
    let limit = max_download_bytes();
    let mut limited = resp.by_ref().take(limit + 1);
    let written = io::copy(&mut limited, &mut out)?;
    out.flush()?;
    drop(out);

    if written > limit {
        let _ = fs::remove_file(&tmp_dest);
        anyhow::bail!("download exceeds max size of {limit} bytes");
    }
    fs::rename(&tmp_dest, dest)?;
    Ok(())
}

/// Opens a fresh regular file, refusing to follow a pre-planted symlink.
fn create_new_file(path: &Path) -> io::Result<File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    match opts.open(path) {
        Ok(f) => Ok(f),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            // Unlink the stale entry (symlink or not) and retry once.
            fs::remove_file(path)?;
            opts.open(path)
        }
        Err(e) => Err(e),
    }
}

/// Private scratch directory under the system temp dir: unpredictable name,
/// 0700, created atomically (fails if the name exists — no symlink following).
fn create_private_workdir(tag: &str) -> anyhow::Result<PathBuf> {
    use rand::RngCore;
    let base = std::env::temp_dir();
    for _ in 0..4 {
        let mut b = [0u8; 6];
        rand::thread_rng().fill_bytes(&mut b);
        let dir = base.join(format!(
            "sub-maker-{tag}-{}-{}",
            std::process::id(),
            hex::encode(b)
        ));
        match fs::create_dir(&dir) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
                }
                return Ok(dir);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    anyhow::bail!("could not create a private temp workdir")
}

pub fn extract_tar_gz(src: &Path, dest: &Path) -> anyhow::Result<()> {
    let f = File::open(src)?;
    let gz = GzDecoder::new(f);
    let mut archive = Archive::new(gz);
    let dest_clean = fs::canonicalize(dest).unwrap_or_else(|_| dest.to_path_buf());

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        // Reject absolute paths and parent traversal.
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir))
        {
            anyhow::bail!("tar entry {:?} attempts path traversal", path);
        }
        let target = dest.join(&path);
        // Extra safety after join — reject if cleaned target escapes dest.
        let target_clean = {
            let mut comps = Vec::new();
            for c in target.components() {
                match c {
                    Component::ParentDir => {
                        if comps.pop().is_none() {
                            anyhow::bail!("tar entry {:?} attempts path traversal", path);
                        }
                    }
                    Component::CurDir => {}
                    other => comps.push(other),
                }
            }
            comps.into_iter().collect::<PathBuf>()
        };
        if !target_clean.starts_with(dest) && target_clean != *dest {
            anyhow::bail!("tar entry {:?} attempts path traversal", path);
        }
        let _ = &dest_clean;

        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.header().entry_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&target)?;
            io::copy(&mut entry, &mut out)?;
            let target_str = target.to_string_lossy();
            if target_str.contains("sing-box") || target_str.contains("subconverter") {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
                }
            }
        }
    }
    Ok(())
}

pub fn download_sing_box(version: &str) -> anyhow::Result<()> {
    let version = if version.is_empty() {
        latest_github_release_tag("SagerNet/sing-box", "v1.12.0")
    } else {
        version.to_string()
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" | "amd64" => "amd64",
        "aarch64" | "arm64" => "arm64",
        _ => "amd64",
    };
    let ver_trim = version.trim_start_matches('v');
    let url = format!(
        "https://github.com/SagerNet/sing-box/releases/download/{version}/sing-box-{ver_trim}-linux-{arch}.tar.gz"
    );
    let work = create_private_workdir("sing-box")?;
    let result = download_sing_box_into(&work, &url);
    let _ = fs::remove_dir_all(&work);
    result
}

fn download_sing_box_into(work: &Path, url: &str) -> anyhow::Result<()> {
    let tmp = work.join("sing-box.tar.gz");
    download_file(url, &tmp).map_err(|e| anyhow::anyhow!("download sing-box: {e}"))?;
    let extract_dir = work.join("extract");
    fs::create_dir_all(&extract_dir)?;
    extract_tar_gz(&tmp, &extract_dir)?;

    let mut bin: Option<PathBuf> = None;
    for entry in walkdir(&extract_dir) {
        if entry.is_file()
            && entry
                .file_name()
                .map(|n| n == "sing-box")
                .unwrap_or(false)
        {
            bin = Some(entry);
            break;
        }
    }
    let bin = bin.ok_or_else(|| anyhow::anyhow!("sing-box binary not found in archive"))?;

    let install_path = if config::demo_mode() {
        let install_dir = PathBuf::from(home_dir()).join(".local/bin");
        fs::create_dir_all(&install_dir)?;
        install_dir.join("sing-box")
    } else {
        PathBuf::from("/usr/local/bin/sing-box")
    };
    fs::rename(bin, &install_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&install_path, fs::Permissions::from_mode(0o755))?;
    }
    info!(path = %install_path.display(), "installed sing-box");
    Ok(())
}

pub fn download_subconverter(version: &str) -> anyhow::Result<()> {
    let version = if version.is_empty() {
        latest_github_release_tag("asdlokj1qpi233/subconverter", "v0.9.9")
    } else {
        version.to_string()
    };
    let asset = subconverter_asset_name();
    let url = format!(
        "https://github.com/asdlokj1qpi233/subconverter/releases/download/{version}/{asset}"
    );
    let work = create_private_workdir("subconverter")?;
    let result = download_subconverter_into(&work, &url);
    let _ = fs::remove_dir_all(&work);
    result
}

fn download_subconverter_into(work: &Path, url: &str) -> anyhow::Result<()> {
    let tmp = work.join("subconverter.tar.gz");
    download_file(url, &tmp).map_err(|e| anyhow::anyhow!("download subconverter: {e}"))?;
    let dest = subconverter_dir();
    let _ = fs::remove_dir_all(&dest);
    fs::create_dir_all(&dest)?;
    extract_tar_gz(&tmp, &dest)
}

fn subconverter_asset_name() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" | "arm64" => "subconverter_aarch64.tar.gz",
        "arm" => "subconverter_armv7.tar.gz",
        "x86" => "subconverter_linux32.tar.gz",
        _ => "subconverter_linux64.tar.gz",
    }
}

pub fn subconverter_dir() -> PathBuf {
    if config::demo_mode() {
        PathBuf::from(home_dir()).join(".local/subconverter")
    } else {
        PathBuf::from("/opt/subconverter")
    }
}

pub fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
}

pub fn subconverter_installed() -> bool {
    let mut candidates = vec![PathBuf::from("/opt/subconverter/subconverter")];
    if config::demo_mode() {
        candidates.insert(
            0,
            PathBuf::from(home_dir()).join(".local/subconverter/subconverter"),
        );
    }
    candidates.iter().any(|p| p.is_file())
}

fn latest_github_release_tag(repo: &str, fallback: &str) -> String {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return fallback.to_string(),
    };
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = match client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "sub-maker")
        .send()
    {
        Ok(r) if r.status().is_success() => r,
        _ => return fallback.to_string(),
    };
    #[derive(serde::Deserialize)]
    struct Payload {
        tag_name: String,
    }
    match resp.json::<Payload>() {
        Ok(p) if !p.tag_name.is_empty() => p.tag_name,
        _ => {
            warn!("could not fetch latest release for {repo}, using {fallback}");
            fallback.to_string()
        }
    }
}

fn walkdir(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.push(path);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    #[test]
    fn create_new_file_refuses_to_write_through_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim");
        fs::write(&victim, "original").unwrap();
        let link = dir.path().join("artifact.tmp");
        std::os::unix::fs::symlink(&victim, &link).unwrap();

        let mut f = create_new_file(&link).unwrap();
        f.write_all(b"evil!").unwrap();
        drop(f);

        assert_eq!(fs::read_to_string(&victim).unwrap(), "original");
        assert!(link.symlink_metadata().unwrap().file_type().is_file());
    }

    #[test]
    fn extract_tar_gz_rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("evil.tar.gz");
        // Craft a minimal ustar entry with a `../` path — tar::Builder refuses this.
        {
            let f = File::create(&archive_path).unwrap();
            let mut enc = GzEncoder::new(f, Compression::default());
            let mut header = [0u8; 512];
            let name = b"../escape.txt";
            header[..name.len()].copy_from_slice(name);
            // mode
            header[100..108].copy_from_slice(b"0000644\0");
            // size = 5
            header[124..136].copy_from_slice(b"00000000005\0");
            // typeflag = regular file
            header[156] = b'0';
            // ustar magic
            header[257..265].copy_from_slice(b"ustar\0  ");
            // checksum (spaces first, then compute)
            header[148..156].copy_from_slice(b"        ");
            let sum: u32 = header.iter().map(|&b| b as u32).sum();
            let cksum = format!("{sum:06o}\0 ");
            header[148..156].copy_from_slice(cksum.as_bytes());
            use std::io::Write;
            enc.write_all(&header).unwrap();
            // file body (padded to 512)
            let mut body = [0u8; 512];
            body[..5].copy_from_slice(b"evil!");
            enc.write_all(&body).unwrap();
            // two zero blocks end tar
            enc.write_all(&[0u8; 1024]).unwrap();
            enc.finish().unwrap();
        }
        let dest = dir.path().join("out");
        fs::create_dir_all(&dest).unwrap();
        let err = extract_tar_gz(&archive_path, &dest);
        assert!(err.is_err(), "expected path traversal error");
        assert!(
            !dir.path().join("escape.txt").exists(),
            "path traversal should not write outside dest"
        );
    }
}
