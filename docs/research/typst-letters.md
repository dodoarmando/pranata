# Rendering letters as PDF with Typst

Research for [#3](https://github.com/dodoarmando/pranata/issues/3): how the API (Rust/axum) should embed Typst to render village letters as PDF.
Checked on 2026-11-18. All versions come from the crates.io API and GitHub releases on that date.

## Answer

Embed Typst in the API process, through the official `typst` + `typst-pdf` crates (0.15.1).
Do not shell out to the CLI.
Start with [`typst-as-lib`](https://crates.io/crates/typst-as-lib) (0.18.0) as the `World` implementation.
It is thin, and if it falls behind a Typst release we can replace it with our own `World` of about 100 lines.
Templates, fonts and the village logo are compiled into the binary or read from one directory.
Letter data goes in as **one JSON string input** that the template decodes with `json(bytes(sys.inputs.data))`.
The same template then runs unchanged under the CLI for template authors (`typst compile --input data=...`).
Every issued letter stores its rendered PDF bytes and the data snapshot, so a reprint never depends on a template or Typst upgrade.

## Current versions

| Crate / artifact       | Version | Released   | License           | Source                               |
| ---------------------- | ------- | ---------- | ----------------- | ------------------------------------ |
| `typst`                | 0.15.1  | 2026-07-17 | Apache-2.0        | crates.io                            |
| `typst-pdf`            | 0.15.1  | 2026-07-17 | Apache-2.0        | crates.io                            |
| `typst-kit`            | 0.15.1  | 2026-07-17 | Apache-2.0        | crates.io                            |
| `typst-assets`         | 0.15.1  | 2026-07-17 | Apache-2.0        | crates.io                            |
| Typst CLI              | v0.15.1 | 2026-07-17 | Apache-2.0        | GitHub releases                      |
| `typst-as-lib`         | 0.18.0  | 2026-11-17 | MIT               | crates.io (depends on `typst ^0.15`) |
| `derive_typst_intoval` | 0.8.0   | 2026-06-26 | MIT OR Apache-2.0 | crates.io                            |
| `typst-bake`           | 0.3.1   | 2026-11-12 | MIT OR Apache-2.0 | crates.io                            |

The Typst 0.15 crates require Rust 1.92 or newer (`rust_version` on crates.io; 0.15.0 changelog).

## Options compared

### A. Embed the `typst` crate (own `World`, or `typst-as-lib`)

The host implements the `World` trait from `typst-library` (re-exported by `typst`).
In 0.15.1 it has seven methods: `library`, `book`, `main`, `source`, `file`, `font`, `today`. It must be `Send + Sync`.
Then it calls `typst::compile::<PagedDocument>(&world)` and `typst_pdf::pdf(&doc, &PdfOptions)`.
[`World` source](https://github.com/typst/typst/blob/v0.15.1/crates/typst-library/src/lib.rs), [`compile` source](https://github.com/typst/typst/blob/v0.15.1/crates/typst/src/lib.rs).

- **API stability.** Typst is pre-1.0, and the Rust API breaks in minor releases.
  0.12 made `World` `Send + Sync` and changed `World::main`. 0.15 "completely reworked" `typst-kit` ([0.12 changelog](https://typst.app/docs/changelog/0.12.0/), [0.15 changelog](https://typst.app/docs/changelog/0.15.0/)).
  The last gap between minor releases was about 8 months (0.14.0 on 2025-10-24, 0.15.0 on 2026-06-15).
  Budget a small migration per minor release and pin the minor version (`typst = "0.15"`).
  The `World` surface itself is small, so most breakage lands in helper crates.
- **Wrapper lag.** `typst-as-lib` 0.16.0 (on `typst ^0.15`) came out 2026-06-29, 14 days after Typst 0.15.0.
  Its README says its API "is currently not really stable". We use only `TypstEngine::builder()`, `.fonts()`, a static file resolver and `compile_with_input`/`world_builder().with_inputs()`, so being exposed to it is cheap.
- **Binary size.** Adds the compiler to the API binary. Not measured here; it should be well under the CLI's 56 MB, because we leave out CLI-only features (package downloads, watcher, HTTP server, embedded default fonts). `typst-kit`'s features are all off by default ([typst-kit Cargo.toml](https://github.com/typst/typst/blob/v0.15.1/crates/typst-kit/Cargo.toml)).
- **Latency.** No process spawn and no font scanning per request. Fonts are parsed once into a `FontBook` that is shared by all renders.
  `comemo` memoization makes repeat renders of the same template cheaper. Long-lived processes must evict the cache (`comemo::evict(n)`), as the CLI does after every compile in watch mode ([watch.rs](https://github.com/typst/typst/blob/v0.15.1/crates/typst-cli/src/watch.rs)).
  `typst-as-lib`'s `with_world` evicts automatically; with `world_builder` the caller does it.
  Compilation is synchronous and CPU-bound, so in axum it runs under `tokio::task::spawn_blocking`.
- **Failure mode.** Errors come back as `SourceDiagnostic`s with spans, and can be logged as structured data.

### B. Shell out to the Typst CLI

`typst compile --ignore-system-fonts --font-path <dir> --input data=<json> letter.typ -` writes the PDF to stdout. Writing to `-` has worked since 0.12 ([0.12 changelog](https://typst.app/docs/changelog/0.12.0/)).

- **API stability.** The CLI flags are more stable than the crate API. Upgrading means replacing a binary.
- **Binary size.** `typst-x86_64-unknown-linux-musl.tar.xz` for v0.15.1 is 17.5 MB compressed, and the `typst` executable inside is 55.7 MB (measured from the release asset). It is a second artifact to ship, version and patch in every Installation's container.
- **Latency.** Each request pays for a process start, and for loading fonts from `--font-path`. Not benchmarked here (running the downloaded binary was blocked in this environment).
  The in-process path avoids that cost by design, so it is faster, but we have no figure for how much.
- **Data injection.** `--input key=value` values are always strings ([`sys`](https://typst.app/docs/reference/foundations/sys/)). The JSON payload would travel on the command line, where `ps` shows it to other users and the OS caps argument length.
  Resident data (NIK, family card number) on a process command line is a privacy regression.
- **Operations.** Temp files or pipes, exit codes, stderr parsing, and zombie and timeout handling are all ours to write.

### C. Higher-level wrappers

`typst-bake` bakes templates, fonts and packages into the binary at build time on top of `typst-as-lib`. `typst-as-lib`'s README calls it "very new" with an unstable API (590 downloads in the last 90 days).
It adds a layer without removing any work that we would otherwise have to do. Skip it.

### Summary

|                     | Embedded (A)                      | CLI (B)                                                              |
| ------------------- | --------------------------------- | -------------------------------------------------------------------- |
| Upgrade cost        | Code changes on Typst minor bumps | Swap binary; flags rarely change                                     |
| Extra artifact      | None                              | 56 MB executable per Installation                                    |
| Per-request cost    | Compile only                      | Process spawn + font load + compile                                  |
| PII exposure        | In memory only                    | Command line or temp file                                            |
| Error handling      | Typed diagnostics                 | stderr text                                                          |
| File/network access | Only what our `World` returns     | CLI defaults (filesystem root, package downloads) unless flagged off |

**Recommendation: A.** It costs more on each upgrade, but that buys one artifact per Installation, no PII on command lines, typed errors, and full control of file and font access.

## Integration sketch

The shape only, not production code.

```toml
# apps/api/Cargo.toml
typst = "0.15"
typst-pdf = "0.15"
typst-as-lib = "0.18"   # or our own World; see "Exit ramp"
```

```rust
// Method names follow the typst-as-lib README; exact signatures not compiled here.
// Built once at startup and shared (Arc) across requests.
static LETTERHEAD: &str = include_str!("../templates/letterhead.typ");
static DOMICILE: &str   = include_str!("../templates/domicile-v1.typ");
static FONTS: &[&[u8]]  = &[include_bytes!("../fonts/LiberationSerif-Regular.ttf"), /* Bold, Italic, BoldItalic */];

let engine = TypstEngine::builder()
    .with_static_source_file_resolver([("letterhead.typ", LETTERHEAD), ("domicile-v1.typ", DOMICILE)])
    .with_static_file_resolver([("logo.png", village_logo_bytes)]) // read once from the Installation's config
    .fonts(FONTS.iter().copied())
    .build();

// Per request, inside spawn_blocking:
let data = serde_json::to_string(&letter_payload)?;          // already validated, already formatted
let inputs = Dict::from_iter([("data".into(), data.into_value())]);
let doc: PagedDocument = engine.compile_with_input("domicile-v1.typ", inputs).output?;
let pdf = typst_pdf::pdf(&doc, &PdfOptions { standards: /* PDF/A-2b, if archival matters */, ..Default::default() })?;
comemo::evict(10); // if not using with_world
```

```typ
// templates/domicile-v1.typ
#import "letterhead.typ": letterhead
#let d = json(bytes(sys.inputs.data))
#set page(paper: "a4")
#set text(font: "Liberation Serif", lang: "id", size: 12pt, hyphenate: false)
#letterhead(d.village)
#align(center)[*SURAT KETERANGAN DOMISILI* \ Nomor: #d.number]
Nama: #d.resident.name \
NIK: #d.resident.nik
```

**Exit ramp.** If `typst-as-lib` stops tracking Typst, swap it for a struct that holds a `LazyHash<Library>` (built with `Library::builder().with_inputs(dict).build()`, see [`LibraryBuilder::with_inputs`](https://github.com/typst/typst/blob/v0.15.1/crates/typst-library/src/lib.rs)), a `LazyHash<FontBook>`, a `Vec<Font>`, and a `HashMap<FileId, Bytes>`. Then implement the seven `World` methods over those fields.
`typst-kit` 0.15's `fonts::FontStore` and `files::FileStore` cover most of it.

## Data injection, safely

- **One JSON input.** `sys.inputs` values are strings ([`sys`](https://typst.app/docs/reference/foundations/sys/)). Pass a single `data` key that holds serialized JSON, and decode it with `json(bytes(...))`.
  `json` takes a path or bytes, so a bare string would be read as a file path ([`json`](https://typst.app/docs/reference/data-loading/json/)).
  JSON `null` becomes `none`, objects become dictionaries. Integers outside ±2^63 become floats.
  **Send NIK and family card numbers as strings**: 16 digits fit in i64, but leading zeros and exact display matter.
  Typed `Dict` values via `IntoValue` (or `derive_typst_intoval`) also work, but the JSON route keeps the template runnable under the CLI for authoring and tests.
- **Data is never code.** A `str` placed in markup renders as literal text, so `*`, `#`, `$`, `<` in a resident's name print as typed.
  The only way to turn data into code is `eval` ([`eval`](https://typst.app/docs/reference/foundations/eval/)). Ban it in templates; a CI grep is enough.
- **Validate before rendering.** The API validates and normalizes letter fields as it does any other input. The template only lays them out.
- **Sandbox by construction.** The template can read only the files our `World::file` returns, and it can reach the network only if a package resolver is enabled.
  Enable none: no remote packages. If a Typst Universe package is ever needed, vendor it into the static resolver.
- **Dates and numbers in Rust.** `datetime.display` prints month and weekday names in English only ([`datetime`](https://typst.app/docs/reference/foundations/datetime/)).
  Format "18 November 2026" and similar Indonesian strings in Rust and pass them as strings.
  Make `World::today` return a fixed value or `None`, so no template depends on the server's clock (`typst-as-lib` exposes a `TimeProvider` for this).
- **Hyphenation.** `lang: "id"` is still correct for PDF tagging and accessibility ([`text.lang`](https://typst.app/docs/reference/text/text/)). But Typst's `hypher` crate has no Indonesian patterns ([hypher README](https://github.com/typst/hypher)), so set `hyphenate: false`, and if text is justified, check the spacing.

## Fonts

- **Bundle, don't discover.** Containers have no useful system fonts, and output must not change with the host.
  Embed font files with `include_bytes!`, or load them from one directory at startup, and never scan system fonts. The CLI equivalent is `--ignore-system-fonts`, which exists "for better reproducibility" ([0.12 changelog](https://typst.app/docs/changelog/0.12.0/)).
- **Offline.** Typst never downloads fonts. With no package resolver, rendering needs no network.
- **Typst's defaults are not letterhead fonts.** `typst-assets` embeds Libertinus Serif, New Computer Modern and DejaVu Sans Mono ([typst-assets](https://github.com/typst/typst-assets/blob/v0.15.1/src/lib.rs)). `text.font` defaults to Libertinus Serif. Every template sets its font explicitly.
- **Office-style letterheads.** Village letters are usually typed in Times New Roman or Arial, which are proprietary and cannot ship in an AGPL repo.
  Liberation Serif / Sans aim at "document layout compatibility" with Times New Roman and Arial, under the SIL Open Font License ([liberation-fonts](https://github.com/liberationfonts/liberation-fonts), latest release 2.1.5). OFL allows bundling with software.
  Keep each font's license file next to it in the repo.
- **Logo.** `image` accepts PNG, JPG, GIF, SVG, PDF and WebP from bytes ([`image`](https://typst.app/docs/reference/visualize/image/)). The village logo can come from the Installation's settings without touching disk.
  PDF images cannot be used together with a PDF/A target, so store the logo as SVG or PNG.

## Template versioning

- Templates are `.typ` files in the repo (`apps/api/templates/`), one per letter type, with the version in the file name (`domicile-v1.typ`), plus shared partials (`letterhead.typ`). They are compiled into the binary, so a release pins templates, Typst version and fonts together.
- A layout change that alters an issued letter's appearance gets a new file (`-v2`). The old one stays until no draft references it.
- Each issued letter records the template id + version, the JSON data snapshot, and the **rendered PDF bytes** (or their hash plus object storage).
  Typst minor upgrades can change layout, so reprints serve the stored PDF and never re-render.
- Per-village content (village name, address, logo, signatory name and title) is data from the Installation's settings, injected through `data`. Templates stay identical across Installations.
- A golden test per template renders a fixed fixture and compares the extracted text, plus a page-count check. This catches Typst upgrade regressions. Pixel comparison is optional.
- Templates editable by admins at runtime are out of scope for v1. If they are needed later, store them in the database with a version and serve them through a custom file resolver. The rules above still hold.

## Licensing

- Typst crates and CLI: Apache-2.0. The ASF states that Apache 2 software "can therefore be included in GPLv3 projects" ([ASF GPL compatibility](https://www.apache.org/licenses/GPL-compatibility.html)). AGPLv3 shares GPLv3's terms, so linking Typst into Pranata (AGPLv3) is fine. Keep Typst's `NOTICE`/attribution in the distributed artifact.
- `typst-as-lib`: MIT, and `derive_typst_intoval`: MIT OR Apache-2.0. Both are compatible.
- Fonts: OFL (Liberation) is fine to redistribute with the app. Proprietary office fonts are not.

## Open points

- Render latency and binary size growth are **not measured**. Measure both with a spike once `apps/api` exists (a typical one-page letter, cold and warm).
- Whether letters must be PDF/A (archival) depends on the letter signing and verification decision on the map (#1). `PdfOptions.standards` supports PDF/A-2b and others ([PDF export](https://typst.app/docs/reference/pdf/)).
- Which fonts the letterhead actually uses depends on the prototype's letter layouts. Check before bundling.

## Sources

- crates.io API: `/api/v1/crates/{typst,typst-pdf,typst-kit,typst-assets,typst-cli,typst-as-lib,derive_typst_intoval,typst-bake}` and their `/versions` and `/dependencies` endpoints (queried 2026-11-18).
- Typst source at tag `v0.15.1`: `crates/typst-library/src/lib.rs` (`World`, `LibraryBuilder`), `crates/typst/src/lib.rs` (`compile`), `crates/typst-pdf/src/lib.rs` (`PdfOptions`), `crates/typst-kit/{Cargo.toml,src/lib.rs,src/fonts.rs,src/files.rs}`, `crates/typst-cli/src/watch.rs`.
- Typst GitHub release v0.15.1 assets: https://github.com/typst/typst/releases/tag/v0.15.1
- Typst docs: https://typst.app/docs/reference/foundations/sys/, https://typst.app/docs/reference/data-loading/json/, https://typst.app/docs/reference/foundations/eval/, https://typst.app/docs/reference/foundations/datetime/, https://typst.app/docs/reference/text/text/, https://typst.app/docs/reference/visualize/image/, https://typst.app/docs/reference/pdf/, https://typst.app/docs/changelog/0.12.0/, https://typst.app/docs/changelog/0.15.0/
- typst-assets `v0.15.1` `src/lib.rs`: https://github.com/typst/typst-assets
- hypher README: https://github.com/typst/hypher
- typst-as-lib README: https://github.com/Relacibo/typst-as-lib
- Liberation Fonts README: https://github.com/liberationfonts/liberation-fonts
- ASF on GPL compatibility: https://www.apache.org/licenses/GPL-compatibility.html
