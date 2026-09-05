use anyhow::{bail, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// True if `fzf` is on PATH.
pub fn is_installed() -> bool {
    Command::new("which")
        .arg("fzf")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Asks the user for permission, then installs fzf via pacman.
pub fn prompt_install() -> Result<()> {
    print!("fzf not found, but it's needed to pick certificate files. Install it now with pacman? [Y/n] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    if answer == "n" || answer == "no" {
        bail!("fzf is required to add certificates");
    }

    let status = Command::new("sudo")
        .args(["pacman", "-S", "--noconfirm", "fzf"])
        .status()?;
    if !status.success() {
        bail!("failed to install fzf via pacman");
    }
    Ok(())
}

/// Runs `find $HOME -type f` piped into `fzf`, letting the user fuzzy-search
/// and pick a certificate file with arrow keys / typing. Returns the chosen
/// path, or None if the user cancelled (Esc in fzf).
pub fn pick_file() -> Result<Option<String>> {
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string());

    let find = Command::new("find")
        .arg(&home)
        .args(["-type", "f"])
        .args([
            "(", "-iname", "*.cer", "-o", "-iname", "*.crt", "-o", "-iname", "*.pem", ")",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let fzf = Command::new("fzf")
        .arg("--prompt=Select certificate: ")
        .arg("--height=40%")
        .arg("--reverse")
        .stdin(find.stdout.context_stdio()?)
        .stdout(Stdio::piped())
        .spawn()?;

    let output = fzf.wait_with_output()?;
    if !output.status.success() {
        // user pressed Esc / Ctrl-C in fzf
        return Ok(None);
    }
    let chosen = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if chosen.is_empty() {
        return Ok(None);
    }
    Ok(Some(chosen))
}

/// Small helper so `find`'s stdout (Option<ChildStdout>) plugs neatly into
/// the next command's Stdio without an extra unwrap at the call site.
trait StdioExt {
    fn context_stdio(self) -> Result<Stdio>;
}
impl StdioExt for Option<std::process::ChildStdout> {
    fn context_stdio(self) -> Result<Stdio> {
        match self {
            Some(out) => Ok(Stdio::from(out)),
            None => bail!("could not capture output of 'find'"),
        }
    }
}
