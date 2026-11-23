# Porting Chromium C++ to Rust: scope, status, and plan

This document records what the tree contains, what the goal "port Chromium to
Rust in its entirety" would involve, and the first verified step. It is kept
in-tree so the next session starts from the same facts.

## Scope (measured from this checkout)

Source: Chromium 160.0.8179.0 (`chrome/VERSION`), git HEAD `bc2dc761fe`.

| Area | Files | Lines (approx.) |
|---|---|---|
| First-party C/C++ (`.cc .h .c .cpp .mm`), excluding `third_party/` | 116,578 | 26.6M |
| `chrome/` (largest) | 35,709 | 8.55M |
| `components/` | 21,443 | 4.60M |
| `ios/` | 13,236 | 1.92M |
| `third_party/` (out of port scope) | 24,476 | 6.4M |

First-party Rust outside `third_party/` is 325 files, mostly in `mojo/`,
`build/`, `components/`, `tools/`, and `base/`. The 7.8k `.rs` files in
`third_party/` are vendored crates.

Because of its size, a full port is a multi-year, multi-team effort. It cannot
be completed in one session.

## Build environment gaps

- 30 `third_party/` directories are empty placeholders for submodules that are
  not initialized (for example `angle`, `icu`, `ffmpeg`, `pdfium`, `libyuv`,
  `depot_tools`). `v8/` is also empty. Fixing this needs `gclient sync`.
- `third_party/rust-toolchain` is absent, so no Rust target builds until
  `tools/rust/update_rust.py` runs after sync. The pinned revision is in
  `DEPS` (`RUST_REVISION`, around lines 1114 to 1152).
- Local tools: `rustc` and `cargo` 1.97.0, `clang`, `ninja`, `python3`. `gn`
  is not installed.
- Machine: 4 CPUs, 15 GB RAM, about 53 GB free disk. A full Chromium build
  needs far more disk and CPU than this.

## Existing Rust/C++ interop mechanism

Chromium already has three bridging mechanisms. A port would use them:

1. **cxx** (`#[cxx::bridge]`), wired through `cxx_bindings` on
   `rust_static_library`, generating `<file>.rs.h`. Examples:
   `base/logging/rust_logger/`, `chrome/utility/safe_browsing/mac/rust/`,
   `components/user_data_importer/utility/parsing_ffi/`.
2. **Crubit**, which generates C++ headers from Rust (`cpp_api_from_rust`) and
   Rust wrappers over C++ headers (`rust_api_from_cpp`). It is enabled only
   when `enable_rust && build_with_chromium` (`build/config/rust.gni:454-456`).
   Examples: `components/cbor/rust/reader.rs`,
   `components/web_package/rust/parser.rs`.
3. **Mojo**, which emits Rust bindings through `generate_rust`
   (`mojo/public/tools/bindings/mojom.gni:1616`), with the runtime in
   `mojo/public/rust`.

GN templates: `rust_static_library`, `rust_shared_library`, `rust_executable`,
`rust_macro`, `rust_unit_test`, `rust_bindgen`, `rust_api_from_cpp`,
`rust_cxx`, and `cargo_crate`. Templates live in `build/rust/` and
`third_party/rust/cxx/chromium_integration/rust_cxx.gni`.

Vendored crates: `third_party/rust/chromium_crates_io/vendor/` has 336 crate
directories, none empty. The `Cargo.toml`, `Cargo.lock`, and `gnrt_config.toml`
live in `chromium_crates_io/`.

## Port strategy

Port leaf units first, then mid-level subsystems, and leave the largest areas
for last. Each unit goes through the same steps:

1. Read the C++ and its unit tests. Write a Rust version that keeps the same
   observable behavior and document any intentional differences.
2. Port the unit tests. Add a randomized or differential test against a simple
   reference when one is possible.
3. Verify with `cargo test` in the crate, and with a GN target once `gclient
   sync` has run.
4. Wire the crate into GN, switch callers over, and delete the C++ once no
   callers remain.

Candidate first units, from the coupling analysis (line counts are
approximate; coupling is measured from `#include` lines, not the build graph):

| Unit | Lines | Notes |
|---|---|---|
| `base/substring_set_matcher` | 1,366 | Pure algorithm, standard library only. **Pilot, done below.** |
| `net/tools/huffman_trie` | 1,386 | Tool code, self-contained |
| `components/metrics/serialization` | 1,166 | Protobuf-adjacent |
| `components/discardable_memory/common` | 1,291 | Small IPC-facing surface |
| `device/udev_linux` | 1,654 | Linux-only, needs libudev bindings |
| `base/numerics` | 4,641 | Heavy templates, harder to port |

## Pilot: `base/substring_set_matcher` (done, verified standalone)

Location: `base/substring_set_matcher/rust/` (standalone crate, not yet wired
into GN).

- Implements the Aho-Corasick matcher from `substring_set_matcher.{h,cc}`:
  `build`, `match_text`, `any_match`, and `is_empty`.
- All 20 tests pass with `cargo test`. They include a port of every case in
  `substring_set_matcher_unittest.cc` and a randomized comparison against a
  brute-force reference (3,000 cases).
- Mutation check: breaking the output-link traversal makes two tests fail, so
  the tests detect a real defect.
- `cargo clippy --all-targets` and `rustfmt --check` are clean.

Known differences from the C++ (also listed in the crate's module docs):
duplicate pattern strings are always ignored, with the first in input order
winning (C++ CHECK-fails in DCHECK builds); duplicate IDs are allowed; nodes
use a plain `Vec`, so memory use is higher than the packed C++ layout;
`EstimateMemoryUsage` is not ported. The ID and node-count limits are kept, so
`build` fails in the same cases as C++.

Not yet done for the pilot: GN integration, switching the C++ callers, and
differential testing against the real C++ build (needs `gclient sync`).

## What is left

The full goal needs, at minimum:

1. Environment: `gclient sync`, the pinned Rust toolchain, and a `gn`/`ninja`
   build of at least one unit test target.
2. Units in the candidate table above, one at a time, each with tests.
3. Mid-level subsystems that depend on those units.
4. The large areas (`content/`, `chrome/`, `ui/`, `components/`), which need
   Crubit or cxx boundaries at scale.
5. Removal of the C++ sources and a final full build and test run.

Steps 2 to 5 cover the bulk of the 26.6M lines. None of them can be finished
in this session.
