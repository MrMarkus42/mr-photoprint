<#
.SYNOPSIS
  Builds everything locally: tests, release .exe, licence list, installer and portable ZIP in dist\.
  (GitHub Actions does the same for releases; this is for trying things out.)
  Needs: Rust, Inno Setup 6.3+ (winget install JRSoftware.InnoSetup), cargo-about (optional).
#>
# Native tools write progress to stderr; failures are caught via $LASTEXITCODE (throw still stops the script).
$ErrorActionPreference = 'Continue'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"').Matches[0].Groups[1].Value
New-Item -ItemType Directory -Force dist | Out-Null

cargo test --locked
if ($LASTEXITCODE -ne 0) { throw 'tests failed' }
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw 'build failed' }

if (Get-Command cargo-about -ErrorAction SilentlyContinue) {
    cargo about generate --locked -o dist\THIRD_PARTY_LICENSES.html about.hbs
    if ($LASTEXITCODE -ne 0) { throw 'cargo about failed' }
} else { Write-Warning 'cargo-about not installed – no licence list (cargo install cargo-about --locked)' }

$iscc = @("$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup not found (winget install JRSoftware.InnoSetup)' }
& $iscc /Qp "/DAppVersion=$version" installer\MrPhotoPrint.iss
if ($LASTEXITCODE -ne 0) { throw 'installer failed' }

$stage = Join-Path dist "MrPhotoPrint-$version"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item target\release\MrPhotoPrint.exe, LICENSE-MIT, LICENSE-APACHE, installer\PORTABLE.txt $stage
if (Test-Path dist\THIRD_PARTY_LICENSES.html) { Copy-Item dist\THIRD_PARTY_LICENSES.html $stage }
Compress-Archive -Force -Path "$stage\*" -DestinationPath "dist\MrPhotoPrint-$version-portable.zip"
Remove-Item -Recurse -Force $stage
Get-ChildItem dist | Format-Table Name, Length
