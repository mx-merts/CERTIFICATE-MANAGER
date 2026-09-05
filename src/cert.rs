use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where certificate-manager keeps its own copy of every certificate it
/// manages. Certificates live here regardless of whether they are active.
/// This always lives under the *invoking user's* home directory — the
/// program should never be launched with `sudo` itself, only specific
/// privileged steps below shell out to `sudo` on their own.
fn store_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().context("could not resolve XDG data dir")?;
    let dir = base.join("certificate-manager").join("certs");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Where the index (id/name/active bookkeeping) is kept.
fn index_path() -> Result<PathBuf> {
    let base = dirs::data_dir().context("could not resolve XDG data dir")?;
    let dir = base.join("certificate-manager");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("index.json"))
}

/// System location the OS actually trusts. A certificate is "active"
/// exactly when a symlink to it exists here.
const TRUST_ANCHOR_DIR: &str = "/etc/ca-certificates/trust-source/anchors";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub id: u32,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    next_id: u32,
    certs: Vec<Certificate>,
}

fn load_index() -> Result<Index> {
    let path = index_path()?;
    if !path.exists() {
        return Ok(Index::default());
    }
    let raw = fs::read_to_string(&path)?;
    if raw.trim().is_empty() {
        return Ok(Index::default());
    }
    let idx: Index = serde_json::from_str(&raw).context("index.json is corrupted")?;
    Ok(idx)
}

fn save_index(idx: &Index) -> Result<()> {
    let path = index_path()?;
    let raw = serde_json::to_string_pretty(idx)?;
    fs::write(path, raw)?;
    Ok(())
}

/// Runs `sudo trust extract-compat` (Arch's ca-certificates refresh command)
/// so the newly (de)activated certificate is picked up system-wide.
fn refresh_system_trust() -> Result<()> {
    let status = Command::new("sudo")
        .arg("trust")
        .arg("extract-compat")
        .status()
        .context("failed to run 'sudo trust extract-compat'")?;
    if !status.success() {
        bail!("'trust extract-compat' exited with a non-zero status");
    }
    Ok(())
}

/// Creates a symlink at `link_path` pointing to `target` inside the
/// root-owned trust anchor directory, via `sudo ln -sf`. This is the
/// piece that actually needs elevated privileges — the rest of the
/// program runs entirely as the normal user.
fn sudo_link(target: &Path, link_path: &Path) -> Result<()> {
    let status = Command::new("sudo")
        .arg("ln")
        .arg("-sf")
        .arg(target)
        .arg(link_path)
        .status()
        .context("failed to run 'sudo ln -sf' (is sudo installed?)")?;
    if !status.success() {
        bail!(
            "could not create symlink at {} — sudo denied or cancelled",
            link_path.display()
        );
    }
    Ok(())
}

/// Removes a symlink from the root-owned trust anchor directory via
/// `sudo rm -f`. `rm -f` does not error if the path is already gone, so
/// this is safe to call even if nothing is active.
fn sudo_unlink(link_path: &Path) -> Result<()> {
    let status = Command::new("sudo")
        .arg("rm")
        .arg("-f")
        .arg(link_path)
        .status()
        .context("failed to run 'sudo rm -f' (is sudo installed?)")?;
    if !status.success() {
        bail!(
            "could not remove symlink at {} — sudo denied or cancelled",
            link_path.display()
        );
    }
    Ok(())
}

pub fn list() -> Result<Vec<Certificate>> {
    Ok(load_index()?.certs)
}

/// Copies a certificate file the user picked (e.g. via fzf) into
/// certificate-manager's own store and registers it in the index. Does not
/// activate it. Never needs sudo — this only touches the user's own
/// ~/.local/share directory.
pub fn add(source_path: &Path) -> Result<Certificate> {
    if !source_path.exists() {
        bail!("file not found: {}", source_path.display());
    }
    let file_name = source_path
        .file_name()
        .context("path has no file name")?
        .to_string_lossy()
        .to_string();

    let mut idx = load_index()?;
    idx.next_id += 1;
    let new_id = idx.next_id;

    let dest = store_dir()?.join(format!("{new_id}_{file_name}"));
    fs::copy(source_path, &dest).with_context(|| {
        format!(
            "could not copy {} into certificate-manager's store",
            source_path.display()
        )
    })?;

    let cert = Certificate {
        id: new_id,
        name: file_name,
        active: false,
    };
    idx.certs.push(cert.clone());
    save_index(&idx)?;
    Ok(cert)
}

/// Activates the given certificate id, deactivating any other certificate
/// that was active (only one certificate is trusted at a time). Prompts
/// for the sudo password if needed.
pub fn activate(id: u32) -> Result<()> {
    let mut idx = load_index()?;

    // deactivate whatever is currently active
    let previously_active: Vec<(u32, String)> = idx
        .certs
        .iter()
        .filter(|c| c.active)
        .map(|c| (c.id, c.name.clone()))
        .collect();
    for (_, name) in &previously_active {
        sudo_unlink(&Path::new(TRUST_ANCHOR_DIR).join(name))?;
    }
    for c in idx.certs.iter_mut() {
        c.active = false;
    }

    let target = idx
        .certs
        .iter_mut()
        .find(|c| c.id == id)
        .context("certificate id not found")?;

    let stored_file = store_dir()?.join(format!("{}_{}", target.id, target.name));
    let link_path = Path::new(TRUST_ANCHOR_DIR).join(&target.name);
    sudo_link(&stored_file, &link_path)?;
    target.active = true;

    save_index(&idx)?;
    refresh_system_trust()?;
    Ok(())
}

/// Deactivates the given certificate id (removes the trust-anchor symlink
/// but keeps the file in certificate-manager's own store).
pub fn deactivate(id: u32) -> Result<()> {
    let mut idx = load_index()?;
    let target = idx
        .certs
        .iter_mut()
        .find(|c| c.id == id)
        .context("certificate id not found")?;

    sudo_unlink(&Path::new(TRUST_ANCHOR_DIR).join(&target.name))?;
    target.active = false;

    save_index(&idx)?;
    refresh_system_trust()?;
    Ok(())
}

/// Permanently removes a certificate: deletes the trust-anchor symlink (if
/// active), deletes the file from certificate-manager's own store, and
/// drops it from the index.
pub fn remove(id: u32) -> Result<Certificate> {
    let mut idx = load_index()?;
    let pos = idx
        .certs
        .iter()
        .position(|c| c.id == id)
        .context("certificate id not found")?;
    let cert = idx.certs.remove(pos);

    if cert.active {
        sudo_unlink(&Path::new(TRUST_ANCHOR_DIR).join(&cert.name))?;
    }

    let stored_file = store_dir()?.join(format!("{}_{}", cert.id, cert.name));
    if stored_file.exists() {
        fs::remove_file(&stored_file)
            .with_context(|| format!("could not delete {}", stored_file.display()))?;
    }

    save_index(&idx)?;
    if cert.active {
        refresh_system_trust()?;
    }
    Ok(cert)
}
