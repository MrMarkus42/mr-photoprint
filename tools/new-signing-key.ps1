<#
.SYNOPSIS
  Creates the release signing key (run ONCE, on the maintainer's own PC).

.DESCRIPTION
  * The secret key goes to %USERPROFILE%\.minisign\mr-photoprint.key, protected by a password
    that minisign asks for. It never goes into the repository (see .gitignore).
  * The public key is written to keys\update-key.pub. Commit that file: it is compiled into the
    program, which then only accepts updates signed with the secret key.
  * Back up the secret key AND its password (e.g. in a password manager). Without them no new
    version can ever be delivered to installed copies.

  Needs minisign:  winget install jedisct1.minisign
#>
# Native tools write progress to stderr; failures are caught via $LASTEXITCODE (throw still stops the script).
$ErrorActionPreference = 'Continue'
$root = Split-Path $PSScriptRoot -Parent
$pub = Join-Path $root 'keys\update-key.pub'
$secDir = Join-Path $env:USERPROFILE '.minisign'
$sec = Join-Path $secDir 'mr-photoprint.key'

if (-not (Get-Command minisign -ErrorAction SilentlyContinue)) { throw 'minisign not found. Install it: winget install jedisct1.minisign' }
if (Test-Path $sec) { throw "A secret key already exists: $sec  (delete it only if you really want a new key - installed copies would no longer accept updates)" }
if ((Get-Content $pub -Raw) -notmatch 'NOT-CONFIGURED') {
    $a = Read-Host 'keys\update-key.pub already holds a real key. Replace it? Installed copies will then reject updates! (yes/no)'
    if ($a -ne 'yes') { exit 1 }
}
New-Item -ItemType Directory -Force $secDir | Out-Null
minisign -G -f -p $pub -s $sec
if ($LASTEXITCODE -ne 0) { throw 'minisign failed' }
Write-Host ''
Write-Host "Secret key : $sec   <- back it up together with its password, never share it" -ForegroundColor Yellow
Write-Host "Public key : $pub   <- commit and push this file" -ForegroundColor Green
Get-Content $pub
