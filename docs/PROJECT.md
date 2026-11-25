# Unprompted: project memory

> 2026-11-25: project renamed from "metadata remover" — Cargo package/lib is now `unprompted` (bin stays `Unprompted`, exe `Unprompted.exe`); folder name `metadta-remover` intentionally unchanged.

> Read this file first if you lost context. It holds the goal, decisions, status, and next steps.

## Goal
A tiny, portable Windows app that removes metadata from AI-generated images.
- Runs from a memory card: one `MetadataRemover.exe`, no installer, no registry, no config files.
- Drag and drop: drop images on the window, or drag them onto the exe.
- Dark, polished UI with custom-drawn icons.
- Originals are never modified. Output is `<name>_clean.<ext>` next to the source.
- Fully tested end to end.

## Supported formats
- PNG: keeps only display-essential chunks (IHDR, PLTE, IDAT, IEND, tRNS, bKGD, gAMA, cHRM, sRGB, iCCP, sBIT, sPLT, acTL, fcTL, fdAT). Drops every text chunk (AI prompts/workflows live here), eXIf, tIME, pHYs, and unknown chunks.
- JPEG: keeps SOF/DHT/DQT/DRI/DNL/DAC/SOS/RST/EOI, JFIF APP0, ICC APP2, and Adobe APP14. Drops EXIF, XMP, IPTC, MPF, C2PA/JUMBF (APP11), COM, other APPn, and any trailing data after EOI.
- WebP: keeps image chunks (VP8, VP8L, ALPH, ANIM, ANMF, ICCP, VP8X). Drops EXIF and XMP chunks, clears their VP8X flags, and recomputes the RIFF size.
- Pixel data is copied byte for byte. No re-encoding, so no quality loss.
- Not supported yet: GIF, HEIC, AVIF, and video.

## Architecture
- `src/lib.rs`: public API. `detect`, `clean`, `clean_file`, and output-path logic.
- `src/formats/{png,jpeg,webp}.rs`: pure byte-level parsers and writers, no dependencies.
- `src/main.rs`: entry point. No args opens the GUI. Args open the GUI with those files queued. `--cli <files>` runs headless and prints a report.
- `src/gui.rs`: egui/eframe app. `src/theme.rs`: dark palette. `src/icons.rs`: vector icons drawn with the painter.
- `build.rs`: embeds `assets/icon.ico` into the exe with winres.
- `scripts/make_icon.py`: regenerates `assets/icon.ico` and `assets/icon.png`.
- `scripts/build.ps1`: release build, copies exe to `dist/` or a `-Destination` (memory card).
- `tests/e2e/run_e2e.py`: end-to-end test. Generates AI-style images with Pillow, runs the real exe, checks results with Pillow.

## Size targets
- Release exe: under 5 MB. Settings: opt-level z, LTO, codegen-units 1, panic abort, strip, static CRT.

## Toolchain notes
- Default Rust toolchain: stable-x86_64-pc-windows-gnu. Windres is available via WinLibs.
- Python 3.13 with Pillow 13 is installed.

## Status
- [x] Memory doc created
- [x] Icon generated (`scripts/make_icon.py` -> `assets/icon.ico` + `assets/icon.png`; old `icon.svg` removed)
- [x] Core library (PNG, JPEG, WebP) + unit tests (21 passing)
- [x] CLI mode (`--cli <files/folders>`, exit codes 0/1/2) + integration tests (3 passing)
- [x] Build script; e2e passing: 42/42 checks
- [x] Portable-run check: exe copied to a temp dir on D: and run there successfully
- [x] MSRV verified: eframe 0.37 / egui 0.37 require Rust 1.95, rfd 1.92 -> Cargo.toml `rust-version = "1.95"`
- [x] **UI/UX redesign to the mockup ("Unprompted" brand, 2026-11-24)**:
  - Renamed app to **Unprompted** (bin `Unprompted.exe`, window title, CLI help). Old MetadataRemover.exe removed.
  - New theme: mockup palette (bg #0B0914, card #131124, panel #1A1733, brand violet #6344f5..#8b5cf6, coral #ff5b5b, emerald #34d399) + Plus Jakarta Sans & JetBrains Mono (subsetted TTFs in `assets/fonts/`, ~215 KB).
  - Layout: title strip, brand header (U logo drawn with painter, Profile dropdown, Advanced popup, gradient Add button), hero drop zone (dashed violet, cloud icon, FAST · LOSSLESS badge, format chips, Browse Computer, Ctrl+O), toolbar (All/Cleaned/Issues tabs with counts, filter search, Detail View toggle, Sort cycle), rich result cards (format tile, status dot+glow, kind chips, size flow with saved chip, output chip, Inspection Report drawer with purged elements, Show in folder, remove), status footer (offline note, cleaned/issues/purged stats, Export All (.zip), Clear list).
  - New engine features: `Profile` (Full / Keep Camera Info / Prompts Only) threaded through all three format cleaners; dependency-free ZIP writer `write_zip` for Export All.
  - Tests: 25 lib + 5 gui/icons + 42 e2e all passing. Exe size 5.3 MB (fonts cost ~300 KB; over the old 5 MB soft target, accepted).
  - Screenshots in `docs/screenshots/`. Design source: `mockup/stitch_modern_website_redesign_showcases/`, brand assets: `public/`.
- [x] **Brand icon + logo fixes (2026-11-25)**:
  - Header logo is now the REAL brand PNG: `assets/logo.rgba` (128×128 RGBA, rounded corners baked, from `public/icon-512.png`) loaded as an egui texture and drawn with `painter.image` (replaces the old painter-drawn U approximation).
  - Exe/taskbar/window icon: `assets/icon.ico` (rounded-corner, 16–256 px) embedded via winres; eframe `ViewportBuilder::with_icon` with `assets/icon256.rgba` gives the window/taskbar icon at runtime.
  - **Bug: cargo drops winres' `cargo:rustc-link-lib`** → icon silently missing from the exe. Fix: build.rs also emits `cargo:rustc-link-arg-bins={OUT_DIR}/resource.o` to link the resource object directly.
  - **Bug: rfd `common-controls-v6` imports `TaskDialogIndirect` from comctl32** — without a Common Controls v6 manifest the exe fails to load (`0xC0000139`), and while the stale loader-error dialog is up it can poison DLL init for other apps. Fix: dropped the feature (we only use `FileDialog`; TaskDialogs unused). Adding the manifest via winres didn't work cleanly on windows-gnu (mingw's `default-manifest.o` is force-appended by the gcc driver → duplicate manifest, warning "multiple non-default manifests").
  - **Debugging lesson:** a raw WGL test (red clear + SwapBuffers) and a minimal stock eframe app were used to isolate the white-screen to loader-lock poisoning from the stale error dialog, not our code. After the fix everything renders normally again.
  - Verified: header shows the real logo, exe/taskbar/title-bar show the brand icon, debug + release both load and paint, 25+5 unit tests + 42/42 e2e pass. Exe size 5.6 MB (icon + RGBA assets add ~0.3 MB).
- [x] **End-to-end human playtest (2026-11-25)** — drove the GUI with real SendInput mouse/keyboard (scripts/playtest.py harness, PrintWindow captures at every step):
  - Verified working: Browse dialog flow (real typed paths, dialog remembers last dir), Ctrl+O shortcut, add files → instant clean cards, collision naming (`_clean2`…`_clean5` across runs), Profile switch re-cleans all items in place (Camera keeps EXIF, C2PA still stripped — verified with a real-world C2PA JPEG from `Downloads/DataSet`), Detail View purged-element drawers (real chunk names + sizes), search filter, tabs, Sort cycle (Added/Name/Size), Export All via real Save dialog (default name `unprompted_clean.zip`, pixel-identical entries, broken file excluded), remove-item X, Clear list, Advanced info popup, Show in folder (opens Explorer /select), error cards for damaged files, drag&drop NOT synthetically tested (OLE drop simulation impractical; logic covered by unit/e2e tests).
  - **Fixed during playtest:** (1) narrow windows (<900 px) clipped header/toolbar/footer controls → raised `ViewportBuilder::with_min_inner_size` to 900×600 (verified live via WM_GETMINMAXINFO reporting 900-pt client minimum); (2) empty result of tab+filter combination showed a blank list with no explanation → added a "Nothing matches here" hint naming the filter with a "Show all files" recovery button (toolbar stays visible; button clears filter + resets tab).
  - Debugging gotchas: the app window got snapped/resized by the user during testing, invalidating absolute click coordinates (re-pin with MoveWindow before each step; stale coords caused false "button dead" conclusions — the Advanced button was always fine). Min-size does not clamp programmatic MoveWindow, only user border drags.

## How to test
- Unit and integration: `cargo test`
- End to end: `python tests/e2e/run_e2e.py` (after `cargo build --release`; exe is now `Unprompted.exe`)
- Build portable exe: `powershell -ExecutionPolicy Bypass -File scripts/build.ps1`

## Decisions log
- Whitelist approach (keep known-safe chunks/segments, drop the rest). Safer than a blacklist for unknown metadata.
- Output never overwrites: a name collision gets `_clean2`, `_clean3`, and so on.
- No CRC recomputation needed: PNG chunks are copied verbatim with their CRCs. WebP has no CRC.
- Profiles keep whole metadata blocks (e.g. the whole EXIF segment), never partial fields — byte-copy safety first.
- ZIP export is stored (no compression) with a hand-rolled CRC-32 writer: no dependencies, bytes stay exact.
- egui 0.37 gotchas: Frames inherit the surrounding RTL layout (force LTR for left-aligned inner content); `ui.menu_button` ignores custom widget colors on this setup — popups are hand-rolled Areas; `TextEdit::frame` takes a Frame, not a bool; `ScrollArea` has no `.spacing()`; style via `ctx.global_style_mut`; `menu_corner_radius` lives on Visuals; `Id::unique` instead of deprecated `Id::new`.
- Header popups close on outside click via `pointer.any_click()` + rect containment check.
- JPEG keeps JFIF APP0 only from APP0 (JFXX thumbnails are dropped), and keeps Adobe APP14 because its colour-transform flag is needed to decode correctly. DHP/EXP (hierarchical JPEG) markers are kept so those files still decode.
- WebP: EXIF/XMP chunks are dropped and the VP8X feature flags for them are cleared in the kept VP8X chunk; the RIFF size is rewritten.
- `expand_inputs` in the lib expands dropped folders to their direct supported image children (sorted, de-duplicated); the GUI and CLI share it.
- egui 0.37 API notes: `App::ui(&mut self, ui, frame)` (no `update`), `Panel::top/bottom(...).show(...)` / `CentralPanel::show(...)` inside a root ui, `Painter::rect_stroke` needs `StrokeKind`, dropped files are `Arc<dyn DroppedFile>` with `.path()`.
- The old `assets/icon.svg` mark was removed; `assets/icon.png` (from `make_icon.py`) is the README icon, `assets/icon.ico` is embedded in the exe.
- GUI screenshots verified by launching the exe, focusing the window with user32 `SetForegroundWindow`, and grabbing the screen with PIL `ImageGrab`.
