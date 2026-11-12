# Security / Sicherheit

## Reporting a vulnerability / Sicherheitslücke melden

Please **do not** open a public issue. Report it privately via
[GitHub Security Advisories](https://github.com/MrMarkus42/mr-photoprint/security/advisories/new).
You'll get an answer as soon as possible; please give us time to fix it before publishing.

Bitte **kein** öffentliches Issue anlegen, sondern privat über
[GitHub Security Advisories](https://github.com/MrMarkus42/mr-photoprint/security/advisories/new)
melden.

Only the latest release is supported with fixes. / Fehlerbehebungen gibt es nur für die neueste
Version.

## How Mr. PhotoPrint protects you

**Photos are untrusted input.** The format is recognised by content, not file name; sizes are
checked before decoding; decoder memory is capped; a crashing decoder becomes an error message.
Folder scans don't follow links; photo lists and imported settings never open network paths.
Details: [docs/LAYOUT.md](docs/LAYOUT.md#protection-against-hostile-files).

**Updates are signed.**

* Releases are signed with [minisign](https://jedisct1.github.io/minisign/) (Ed25519). The secret
  key exists only offline on the maintainer's PC – never on GitHub. The public key is compiled into
  the program; it is not read from any file or registry value.
* The signature's trusted comment binds file kind and version, and only versions newer than the
  running one are accepted, so an older (genuinely signed) installer can't be replayed.
* The downloaded installer is locked against changes from the final signature check until Windows
  has started it (no swapping before the UAC prompt).
* HTTPS only (TLS 1.2+, Windows' own WinHTTP), size limits, only this project's release URLs.
* The program sends nothing but the request for the release list and its files. No telemetry.
* Release builds are attested by GitHub Actions (build provenance).

## Verifying downloads / Downloads prüfen

<a id="verifying-downloads"></a><a id="downloads-prüfen"></a>

Every release has `SHA256SUMS.txt`, a `.minisig` signature per file and a build attestation.

```powershell
# checksum / Prüfsumme
Get-FileHash .\MrPhotoPrint-Setup-1.0.0.exe -Algorithm SHA256

# signature / Signatur (winget install jedisct1.minisign)
minisign -V -P RWTBro1YLFoI+4CDiri+hGwNpoOAGVv3ipvwz6wjyQDg8tQSwwUAG3mi -m .\MrPhotoPrint-Setup-1.0.0.exe

# build provenance (winget install GitHub.cli)
gh attestation verify .\MrPhotoPrint-Setup-1.0.0.exe --repo MrMarkus42/mr-photoprint
```

Release signing key (minisign) – also in [`keys/update-key.pub`](keys/update-key.pub):

```
RWTBro1YLFoI+4CDiri+hGwNpoOAGVv3ipvwz6wjyQDg8tQSwwUAG3mi
```
