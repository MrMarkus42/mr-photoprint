<#
.SYNOPSIS
  Signs a release that GitHub Actions has built, and publishes it.

.DESCRIPTION
  1. Downloads installer, portable ZIP and SHA256SUMS.txt of the draft release v<Version>.
  2. Checks the checksums and GitHub's build attestation (the files were built by this
     repository's release workflow from the tagged commit – not uploaded by someone else).
  3. Signs both files with the secret key on THIS PC (minisign asks for the password). The
     trusted comment names file kind and version, so a signature can't be reused for another file.
  4. Checks the signatures with keys\update-key.pub (the key compiled into the program).
  5. Uploads the .minisig files and – after asking – publishes the release.

  Needs: gh (logged in), minisign.   Usage:  .\tools\sign-release.ps1 1.1.0
#>
param([Parameter(Mandatory = $true)][string]$Version)
# Native tools write progress to stderr; failures are caught via $LASTEXITCODE (throw still stops the script).
$ErrorActionPreference = 'Continue'
$repo = 'MrMarkus42/mr-photoprint'
$root = Split-Path $PSScriptRoot -Parent
$pub = Join-Path $root 'keys\update-key.pub'
$sec = Join-Path $env:USERPROFILE '.minisign\mr-photoprint.key'
$tag = "v$Version"
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Version must look like 1.2.3' }
foreach ($c in 'gh', 'minisign') { if (-not (Get-Command $c -ErrorAction SilentlyContinue)) { throw "$c not found" } }
if (-not (Test-Path $sec)) { throw "Secret key not found: $sec (run tools\new-signing-key.ps1 first)" }
if ((Get-Content $pub -Raw) -match 'NOT-CONFIGURED') { throw 'keys\update-key.pub is still the placeholder' }

$setup = "MrPhotoPrint-Setup-$Version.exe"
$zip = "MrPhotoPrint-$Version-portable.zip"
$work = Join-Path $env:TEMP "mr-photoprint-sign-$Version"
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory $work | Out-Null

Write-Host "Tag $tag points to:" -ForegroundColor Cyan
gh api "repos/$repo/commits/$tag" --jq '.sha + "  " + (.commit.message | split("\n")[0])'
gh release download $tag --repo $repo --dir $work --pattern $setup --pattern $zip --pattern 'SHA256SUMS.txt'
if ($LASTEXITCODE -ne 0) { throw 'download failed – does the draft release exist?' }

Write-Host 'Checking checksums …' -ForegroundColor Cyan
foreach ($line in Get-Content (Join-Path $work 'SHA256SUMS.txt')) {
    $hash, $name = $line -split '\s+\*?', 2
    if (-not $name) { continue }
    $actual = (Get-FileHash (Join-Path $work $name) -Algorithm SHA256).Hash
    if ($actual -ne $hash.ToUpper()) { throw "checksum mismatch: $name" }
    Write-Host "  ok  $name"
}

Write-Host 'Checking build attestation …' -ForegroundColor Cyan
foreach ($f in $setup, $zip) {
    gh attestation verify (Join-Path $work $f) --repo $repo
    if ($LASTEXITCODE -ne 0) { throw "no valid build attestation for $f – do NOT sign it" }
}

Write-Host 'Signing (minisign asks for your key password) …' -ForegroundColor Cyan
minisign -S -s $sec -m (Join-Path $work $setup) -t "mr-photoprint-setup $Version"
if ($LASTEXITCODE -ne 0) { throw 'signing failed' }
minisign -S -s $sec -m (Join-Path $work $zip) -t "mr-photoprint-portable $Version"
if ($LASTEXITCODE -ne 0) { throw 'signing failed' }
foreach ($f in $setup, $zip) {
    minisign -V -p $pub -m (Join-Path $work $f)
    if ($LASTEXITCODE -ne 0) { throw "signature check failed for $f – is keys\update-key.pub the matching public key?" }
}

gh release upload $tag --repo $repo --clobber (Join-Path $work "$setup.minisig") (Join-Path $work "$zip.minisig")
if ($LASTEXITCODE -ne 0) { throw 'upload failed' }
Write-Host "Signed and uploaded. Draft: https://github.com/$repo/releases/tag/$tag" -ForegroundColor Green
$a = Read-Host 'Publish the release now? (yes/no)'
if ($a -eq 'yes') {
    gh release edit $tag --repo $repo --draft=false --latest
    Write-Host 'Published. Installed copies will find it at their next check.' -ForegroundColor Green
}
