# certificate-manager

A small interactive CLI for managing custom SSL/TLS certificates on Arch
Linux based systems — add, activate/deactivate, and permanently remove
certificates from a boxed terminal menu.

## Requirements

- Arch Linux or an Arch-based distro (trust anchors live under
  `/etc/ca-certificates/trust-source/anchors`)
- `fzf` (installed automatically via pacman on first run if missing)
- `sudo` privileges (needed to (de)activate certificates system-wide)

## Build from source

```bash
cargo build --release
sudo install -Dm755 target/release/certificate-manager /usr/local/bin/certificate-manager
```

## Usage

```bash
certificate-manager
```

- **UP/DOWN** — move the selection
- **ENTER** — confirm / toggle
- **ESC** — go back
- **Q** — quit from the main menu

### Flow

1. Running `certificate-manager` shows every certificate you've added, with its status
   (`ACTIVE`/`INACTIVE`), a stable id, and its file name:
   `[ACTIVE]-[12]-[my_cert.cer]`
2. **ADD** launches `fzf` over your home directory so you can fuzzy-pick a
   `.cer` / `.crt` / `.pem` file. It's copied into `certificate-manager`'s own store at
   `~/.local/share/certificate-manager/certs/`.
3. **SELECT** lets you toggle a certificate on or off. Only one certificate
   is ever active at a time — activating one deactivates whatever was active
   before it. Activating creates a symlink in
   `/etc/ca-certificates/trust-source/anchors/` and runs
   `sudo trust extract-compat` to refresh the system trust store.
4. **REMOVE** asks for confirmation, then deletes the certificate from both
   the trust anchors (if active) and `certificate-manager`'s own store. This cannot be undone.

## How data is stored

- `~/.local/share/certificate-manager/certs/` — every certificate `certificate-manager` manages, active or not
- `~/.local/share/certificate-manager/index.json` — id / name / active bookkeeping (keeps ids
  stable even as certificates are added and removed)
- `/etc/ca-certificates/trust-source/anchors/` — only ever contains a
  symlink to whichever certificate is currently active

## Use cases

Anywhere you need to install a custom root/intermediate certificate and
toggle it on or off without digging through `trust` commands by hand —
corporate proxies, self-signed development certificates, VPNs, captive
networks that require SSL inspection, and similar setups.
