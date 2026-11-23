# Rust port status

Companion to `PLAN.md`. This file records what has been ported and how each
crate was checked. Update it whenever a wave of crates is verified.

## Summary (after wave 1)

- 22 standalone, std-only crates, about 29,500 lines of Rust.
- 722 unit tests plus 1 integration test, all passing.
- `cargo clippy --all-targets` reports no warnings and `rustfmt --check` is
  clean for every crate.
- None of the crates is wired into GN yet, and no C++ has been removed or
  replaced. Chromium builds exactly as before.
- Scale check: the ported units total roughly 27,000 lines of C++ including
  their tests, against about 26.6 million first-party lines. That is about
  0.1% of the code base.

## How to re-verify

Each crate is its own cargo package (with an empty `[workspace]` table so cargo
does not look for a parent workspace). From a crate directory:

```
cargo test && cargo clippy --all-targets && rustfmt --check --edition 2021 src/*.rs
```

## Verification levels

1. **Ported C++ unit tests.** Every crate ports the cases from the matching
   `*_unittest.cc`, except where the table below says otherwise.
2. **Randomized or differential tests against a Rust reference.** Most crates
   add these.
3. **Differential tests against the real C++.**
   - `prtime`: `tests/cpp_golden.rs` holds 1,808 rows (904 inputs in UTC and
     UTC-8) produced by compiling the unmodified `prtime.cc` with stand-in
     headers for `base/check.h` and `build/build_config.h`. That C++ was
     rebuilt a second time independently and every row replayed with 0
     mismatches.
   - `pattern`, `levenshtein_distance`, `hash`: the porting agent reports 0
     mismatches over 40k, 30k, and 4k inputs against C++ built the same way.
     Those runs were not repeated independently.
4. No C++ has been built or run for any other crate. Their expected values
   come from the C++ tests and from reading the source.

## Crates

| Crate (path) | Tests | C++ cases ported | Gaps and intentional differences |
|---|---|---|---|
| `base/substring_set_matcher/rust` | 20 | all | Plain `Vec` nodes (more memory); duplicate strings always ignored, first wins; no `EstimateMemoryUsage` |
| `base/strings/rust/pattern` | 7 | 1 of 1 | `pattern_fuzzer.cc` not ported |
| `base/strings/rust/levenshtein_distance` | 7 | 2 of 2 | `k` clamped to the longer length |
| `base/hash/rust/hash` | 13 | 4 of 4 | No CityHash128, seeded variants, or DCHECK address scramble; `HashCombine` uses Rust `Hash` |
| `base/rust/base64` | 29 | 19 of 19 | No EarlyStartup variants, `*ForTesting` hooks, or `NoPaddingValidation`; fuzz tests are seeded randomized tests |
| `base/rust/version` | 13 | 9 of 9 | Comparisons return `Option` where C++ DCHECKs |
| `base/rust/uuid` | 15 | 8 of 8 | Random UUIDs take caller-provided bytes; no Android JNI conversions |
| `base/metrics/rust/bucket_ranges` | 10 | 4 of 4 | CRC32 reimplemented, known checksums match |
| `base/strings/rust/string_split` | 78 | all (25 + 32 typed) | Separators are `char`; empty-key key/value pairs count as complete, following the code rather than the stale header comment |
| `base/strings/rust/string_number_conversions` | 32 | 19 of 19 | Windows and abseil variants not ported; `StringToDouble` uses Rust's correctly rounded parser on the exact digit string |
| `base/strings/rust/utf_string_conversions` | 38 | 20 of 20 | `icu_utf.cc` is absent from this checkout, so the UTF macros come from `icu_utf.h`; 16-bit `wchar_t` variants not ported |
| `base/strings/rust/escape` | 32 | 15 of 15 | `OffsetAdjuster` adjust/unadjust helpers and the `u16` `EscapeForHTML` not ported |
| `base/files/rust/file_path` | 45 | 34 of 38 | Skipped: Android content URI, perfetto tracing, two Apple-only tests. Paths are UTF-8 `String`s; Windows case folding uses Rust simple uppercase |
| `base/third_party/nspr/rust/prtime` | 48 + 1 | 24 of 24 | Local time is a caller-supplied offset function; `PR_ExplodeTime` and `PR_LocalTimeParameters` are not in this checkout's `prtime.{h,cc}` |
| `net/base/rust/ip_address` | 69 | 45 of 45 | `ParseCIDRBlock` returns `None` on any failure; no socket conversions; `base::Value` replaced by a small enum |
| `net/base/rust/host_port_pair` | 17 | 10 of 15 | `FromURL`, `FromIPEndPoint`, `FromValue`/`ToValue` not ported (5 tests); `ComputeBounds` saturates where C++ overflows |
| `net/http/rust/http_byte_range` | 11 | 3 of 3 | none noted |
| `net/http/rust/http_status_code` | 6 | 1 of 1 | none noted |
| `net/http/rust/http_util` | 77 | 55 of 56 + fuzz | `ParseRangeHeader` rejects `bytes=0--1`, which the C++ `-1` sentinel accepts. `RequestUrlSanitize`, `SpecForRequest`, `GenerateRequestLine` (need GURL) not ported; HTTP-date parser is narrower than `base::Time` and UTC only |
| `ui/gfx/geometry/rust/gfx_geometry` | 106 | 70 of 70 | `GetNormalized` returns `Option`; `Size::area` panics on overflow (`checked_area` returns `Option`); no Rect, Insets, Transform yet |
| `ui/gfx/geometry/rust/cubic_bezier` | 20 | 10 of 10 | Reads of uninitialized `t0`/`t1` in the C++ solver use the last segment instead |
| `ui/gfx/geometry/rust/quaternion` | 29 | 18 of 18 | Uses a private `Vec3` instead of `gfx_geometry`'s `Vector3dF`; should switch once the crates are linked |

## Known follow-ups

- The `http_util` `bytes=0--1` difference should be resolved by matching the
  C++ acceptance behavior or documenting why rejecting is intended.
- Replace `quaternion`'s private `Vec3` with `gfx_geometry::Vector3dF`.
- GN integration: wrap each crate with `rust_static_library`, add a
  `rust_unit_test`, and bridge callers through cxx or Crubit. This needs
  `gclient sync` and `gn`, which are not available in this environment.
- Differential-test more crates against compiled C++, using the stand-in-header
  approach that worked for `prtime`.
- Remaining C++ in the same directories (for example the rest of
  `ui/gfx/geometry`, `net/http`, `base/strings`) is not yet ported.
