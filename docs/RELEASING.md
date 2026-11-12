# Releasing a new version

Releases are built by GitHub Actions but **signed on the maintainer's own PC**. The signing key
never touches GitHub, so even someone who took over the GitHub account could not ship an update
that installed copies accept.

## One-time setup

```powershell
winget install jedisct1.minisign
winget install GitHub.cli
gh auth login
.\tools\new-signing-key.ps1
```

`new-signing-key.ps1` asks for a password and creates

* `%USERPROFILE%\.minisign\mr-photoprint.key` – the **secret** key. Back it up together with its
  password (e.g. in a password manager). Losing it means installed copies can never be updated
  automatically again; leaking it means someone else could sign updates.
* `keys\update-key.pub` – the **public** key. Commit and push it; it is compiled into the program.

Builds made before the key existed contain a placeholder and never install updates.

## Each release

1. Update `version` in `Cargo.toml` and add a section `## [x.y.z] – date` to `CHANGELOG.md`
   (its text becomes the release notes, shown in the program's update window).
2. Commit, then tag and push:
   ```powershell
   git tag v1.1.0
   git push origin main v1.1.0
   ```
3. The **Release** workflow tests, builds the installer, the portable ZIP, `SHA256SUMS.txt` and the
   licence list, attests the build and creates a **draft** release. Drafts are invisible to the
   program's updater.
4. Sign and publish:
   ```powershell
   .\tools\sign-release.ps1 1.1.0
   ```
   The script shows the tagged commit, checks the checksums and GitHub's build attestation, signs
   both files (trusted comment `mr-photoprint-setup 1.1.0` / `mr-photoprint-portable 1.1.0`),
   verifies the signatures with `keys\update-key.pub`, uploads the `.minisig` files and – after
   asking – publishes the release.

## What the program checks

* Only the release marked *latest*, not a draft or pre-release, with a higher version number.
* Only files from `https://github.com/MrMarkus42/mr-photoprint/releases/download/…`.
* The installer's minisign signature against the compiled-in public key (pre-hashed Ed25519).
* The signature's trusted comment must be exactly `mr-photoprint-setup <version of the release>`.
* The installer is held open (no writing or deleting allowed for others) from the final check
  until Windows has started it.

## Building locally

```powershell
.\tools\build-local.ps1
```

creates the same files in `dist\` (unsigned) – useful for testing the installer.
