# Contributing

Thanks for helping! Bug reports, ideas and pull requests are welcome – in English or German.

## Building

You need Windows 10/11 and a current stable [Rust](https://rustup.rs) toolchain (MSVC).

```powershell
git clone https://github.com/MrMarkus42/mr-photoprint
cd mr-photoprint
cargo run                 # debug build, starts the program
cargo test                # all tests
cargo build --release     # target\release\MrPhotoPrint.exe
```

The release build is optimised for size (fat LTO, one codegen unit) and takes a few minutes.
`.\tools\build-local.ps1` additionally builds the installer (needs
`winget install JRSoftware.InnoSetup`) and the portable ZIP.

Useful switches: `MrPhotoPrint.exe --lang en|de` (language), `--size 1280x800` (window size),
`--screenshot out.png` (renders a screenshot and exits), followed by photo files or folders.

## Code layout

| File | What it does |
|---|---|
| `src/main.rs` | Start-up, command line |
| `src/app.rs` | Main window: top bar, gallery, print setup |
| `src/settings_ui.rs` | Settings window, help, about, sheet preview |
| `src/layout.rs` | Where photos land on the paper ([docs/LAYOUT.md](docs/LAYOUT.md)) |
| `src/render.rs` | Safe photo loading, thumbnails, drawing sheets |
| `src/printer.rs` | Printers, papers, the print job (Windows GDI), JPEG export |
| `src/model.rs` | Settings and their limits |
| `src/update.rs`, `src/update_ui.rs` | Signed updates from GitHub |
| `src/i18n.rs` | German/English |
| `src/log.rs` | Local log file |
| `src/theme.rs`, `src/widgets.rs`, `src/icons.rs`, `src/logo.rs` | Look and feel |

## Guidelines

* Every text on screen in both languages: `t("Deutsch", "English")`.
* Treat every file and every downloaded byte as hostile; add a test for each check.
* Keep it small: think twice before adding a dependency.
* `cargo test` must pass. New behaviour comes with a test.
* By contributing you agree that your work is licensed under MIT OR Apache-2.0, like the project.

## Releases

See [docs/RELEASING.md](docs/RELEASING.md).
