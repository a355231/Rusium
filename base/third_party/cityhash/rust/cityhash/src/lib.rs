// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of CityHash v1.1.1 as shipped in
//! `base/third_party/cityhash/city.{h,cc}` (the `base::internal::cityhash_v111`
//! namespace).
//!
//! Ported functions:
//! - [`cityhash64`] (`CityHash64`)
//! - [`cityhash64_with_seed`] (`CityHash64WithSeed`)
//! - [`cityhash64_with_seeds`] (`CityHash64WithSeeds`)
//! - [`cityhash128`] (`CityHash128`)
//! - [`cityhash128_with_seed`] (`CityHash128WithSeed`)
//! - [`cityhash32`] (`CityHash32`; it is declared in `city.h` and implemented
//!   in `city.cc`, so it is included even though it was not listed separately)
//! - [`hash128_to_64`] (`Hash128to64`), [`uint128_low64`] and
//!   [`uint128_high64`] (`Uint128Low64` / `Uint128High64`).
//!
//! Output is bit-exact with the C++ code. The golden vectors in
//! `tests/cpp_golden.rs` were generated from the unmodified `city.cc` built
//! with g++ on little-endian x86-64.
//!
//! Differences from the C++ version:
//! - Inputs are `&[u8]`, so the length is the slice length. The C++ `(ptr,
//!   len)` pairs are not needed.
//! - `Uint128` is a `(u64, u64)` tuple, where `.0` is the low 64 bits and `.1`
//!   is the high 64 bits. This matches `std::pair<uint64, uint64>` in C++.
//! - Every 64-bit and 32-bit word is read as little-endian. The C++ code reads
//!   native words on little-endian targets and byte-swaps on big-endian
//!   targets, so the defined output is the same little-endian result on both.
//!   The C++ comment says big-endian builds are lightly tested.
//! - The `bswap_*` macro zoo, `LIKELY`, `UNALIGNED_LOAD*` and `PERMUTE3` are
//!   replaced by `swap_bytes`, plain code, `from_le_bytes` loads and a local
//!   rotation of three values.
//! - Arithmetic is wrapping, as in C++ unsigned arithmetic.
//!
//! Not ported:
//! - `citycrc.h` / `CityHashCrc128*` and `CityHashCrc256`: not present in the
//!   checkout, and they need SSE4.2 CRC intrinsics.
//! - The deprecated v1.0.3 copy in `base/third_party/cityhash_v103` is a
//!   different directory and has different hash values. It is not ported here.
//!
//! The crate uses only `std` and contains no `unsafe` code.

#![forbid(unsafe_code)]

/// A 128-bit value as `(low64, high64)`, like `std::pair<uint64, uint64>`.
pub type Uint128 = (u64, u64);

/// Returns the low 64 bits of `x` (`Uint128Low64`).
#[inline]
pub fn uint128_low64(x: Uint128) -> u64 {
    x.0
}

/// Returns the high 64 bits of `x` (`Uint128High64`).
#[inline]
pub fn uint128_high64(x: Uint128) -> u64 {
    x.1
}

// Some primes between 2^63 and 2^64.
const K0: u64 = 0xc3a5c85c97cb3127;
const K1: u64 = 0xb492b66fbe98f273;
const K2: u64 = 0x9ae16a3b2f90404f;

// Magic numbers for 32-bit hashing. Copied from Murmur3.
const C1: u32 = 0xcc9e2d51;
const C2: u32 = 0x1b873593;

/// Reads 8 bytes at `i` as little-endian (`Fetch64`).
#[inline]
fn fetch64(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().expect("8-byte slice"))
}

/// Reads 4 bytes at `i` as little-endian (`Fetch32`).
#[inline]
fn fetch32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().expect("4-byte slice"))
}

/// Murmur3 32-bit finalizer (`fmix`).
#[inline]
fn fmix(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h
}

/// Right-rotates `val` by `shift` bits (`Rotate32`).
#[inline]
fn rotate32(val: u32, shift: u32) -> u32 {
    val.rotate_right(shift)
}

/// Murmur3-style combiner (`Mur`).
#[inline]
fn mur(a: u32, h: u32) -> u32 {
    let mut a = a.wrapping_mul(C1);
    a = rotate32(a, 17);
    a = a.wrapping_mul(C2);
    let h = rotate32(h ^ a, 19);
    h.wrapping_mul(5).wrapping_add(0xe6546b64)
}

fn hash32_len13_to_24(s: &[u8]) -> u32 {
    let len = s.len();
    let a = fetch32(s, (len >> 1) - 4);
    let b = fetch32(s, 4);
    let c = fetch32(s, len - 8);
    let d = fetch32(s, len >> 1);
    let e = fetch32(s, 0);
    let f = fetch32(s, len - 4);
    let h = len as u32;

    fmix(mur(f, mur(e, mur(d, mur(c, mur(b, mur(a, h)))))))
}

fn hash32_len0_to_4(s: &[u8]) -> u32 {
    let mut b: u32 = 0;
    let mut c: u32 = 9;
    for &byte in s {
        // C++ uses `signed char`, so the byte is sign-extended.
        let v = (byte as i8 as i32) as u32;
        b = b.wrapping_mul(C1).wrapping_add(v);
        c ^= b;
    }
    fmix(mur(b, mur(s.len() as u32, c)))
}

fn hash32_len5_to_12(s: &[u8]) -> u32 {
    let len = s.len();
    let mut a = len as u32;
    let mut b = a.wrapping_mul(5);
    let mut c: u32 = 9;
    let d = b;
    a = a.wrapping_add(fetch32(s, 0));
    b = b.wrapping_add(fetch32(s, len - 4));
    c = c.wrapping_add(fetch32(s, (len >> 1) & 4));
    fmix(mur(c, mur(b, mur(a, d))))
}

/// 32-bit hash of a byte array (`CityHash32`). Suited to 32-bit targets.
pub fn cityhash32(s: &[u8]) -> u32 {
    let len = s.len();
    if len <= 24 {
        return if len <= 12 {
            if len <= 4 {
                hash32_len0_to_4(s)
            } else {
                hash32_len5_to_12(s)
            }
        } else {
            hash32_len13_to_24(s)
        };
    }

    // len > 24
    let mut h = len as u32;
    let mut g = C1.wrapping_mul(h);
    let mut f = g;
    let mut a0 = rotate32(fetch32(s, len - 4).wrapping_mul(C1), 17).wrapping_mul(C2);
    let mut a1 = rotate32(fetch32(s, len - 8).wrapping_mul(C1), 17).wrapping_mul(C2);
    let mut a2 = rotate32(fetch32(s, len - 16).wrapping_mul(C1), 17).wrapping_mul(C2);
    let mut a3 = rotate32(fetch32(s, len - 12).wrapping_mul(C1), 17).wrapping_mul(C2);
    let mut a4 = rotate32(fetch32(s, len - 20).wrapping_mul(C1), 17).wrapping_mul(C2);
    h ^= a0;
    h = rotate32(h, 19);
    h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
    h ^= a2;
    h = rotate32(h, 19);
    h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
    g ^= a1;
    g = rotate32(g, 19);
    g = g.wrapping_mul(5).wrapping_add(0xe6546b64);
    g ^= a3;
    g = rotate32(g, 19);
    g = g.wrapping_mul(5).wrapping_add(0xe6546b64);
    f = f.wrapping_add(a4);
    f = rotate32(f, 19);
    f = f.wrapping_mul(5).wrapping_add(0xe6546b64);
    let mut pos = 0usize;
    let mut iters = (len - 1) / 20;
    loop {
        a0 = rotate32(fetch32(s, pos).wrapping_mul(C1), 17).wrapping_mul(C2);
        a1 = fetch32(s, pos + 4);
        a2 = rotate32(fetch32(s, pos + 8).wrapping_mul(C1), 17).wrapping_mul(C2);
        a3 = rotate32(fetch32(s, pos + 12).wrapping_mul(C1), 17).wrapping_mul(C2);
        a4 = fetch32(s, pos + 16);
        h ^= a0;
        h = rotate32(h, 18);
        h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
        f = f.wrapping_add(a1);
        f = rotate32(f, 19);
        f = f.wrapping_mul(C1);
        g = g.wrapping_add(a2);
        g = rotate32(g, 18);
        g = g.wrapping_mul(5).wrapping_add(0xe6546b64);
        h ^= a3.wrapping_add(a1);
        h = rotate32(h, 19);
        h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
        g ^= a4;
        g = g.swap_bytes().wrapping_mul(5);
        h = h.wrapping_add(a4.wrapping_mul(5));
        h = h.swap_bytes();
        f = f.wrapping_add(a0);
        // PERMUTE3(f, h, g): (f, h, g) <- (g, f, h).
        let (nf, nh, ng) = (g, f, h);
        f = nf;
        h = nh;
        g = ng;
        pos += 20;
        iters -= 1;
        if iters == 0 {
            break;
        }
    }
    g = rotate32(g, 11).wrapping_mul(C1);
    g = rotate32(g, 17).wrapping_mul(C1);
    f = rotate32(f, 11).wrapping_mul(C1);
    f = rotate32(f, 17).wrapping_mul(C1);
    h = rotate32(h.wrapping_add(g), 19);
    h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
    h = rotate32(h, 17).wrapping_mul(C1);
    h = rotate32(h.wrapping_add(f), 19);
    h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
    h = rotate32(h, 17).wrapping_mul(C1);
    h
}

/// Bitwise right rotate (`Rotate`).
#[inline]
fn rotate(val: u64, shift: u32) -> u64 {
    val.rotate_right(shift)
}

#[inline]
fn shift_mix(val: u64) -> u64 {
    val ^ (val >> 47)
}

/// Hash128to64 applied to `(u, v)`, i.e. `HashLen16(u, v)` in the C++.
#[inline]
fn hash_len16(u: u64, v: u64) -> u64 {
    hash128_to_64((u, v))
}

#[inline]
fn hash_len16_mul(u: u64, v: u64, mul: u64) -> u64 {
    // Murmur-inspired hashing.
    let mut a = (u ^ v).wrapping_mul(mul);
    a ^= a >> 47;
    let mut b = (v ^ a).wrapping_mul(mul);
    b ^= b >> 47;
    b.wrapping_mul(mul)
}

/// Hashes 128 input bits down to 64 bits (`Hash128to64`). This is intended to
/// be a reasonably good hash function.
pub fn hash128_to_64(x: Uint128) -> u64 {
    // Murmur-inspired hashing.
    const K_MUL: u64 = 0x9ddfea08eb382d69;
    let mut a = (uint128_low64(x) ^ uint128_high64(x)).wrapping_mul(K_MUL);
    a ^= a >> 47;
    let mut b = (uint128_high64(x) ^ a).wrapping_mul(K_MUL);
    b ^= b >> 47;
    b.wrapping_mul(K_MUL)
}

fn hash_len0_to_16(s: &[u8]) -> u64 {
    let len = s.len();
    if len >= 8 {
        let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
        let a = fetch64(s, 0).wrapping_add(K2);
        let b = fetch64(s, len - 8);
        let c = rotate(b, 37).wrapping_mul(mul).wrapping_add(a);
        let d = rotate(a, 25).wrapping_add(b).wrapping_mul(mul);
        return hash_len16_mul(c, d, mul);
    }
    if len >= 4 {
        let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
        let a = fetch32(s, 0) as u64;
        return hash_len16_mul((len as u64).wrapping_add(a << 3), fetch32(s, len - 4) as u64, mul);
    }
    if len > 0 {
        let a = s[0];
        let b = s[len >> 1];
        let c = s[len - 1];
        let y = (a as u32).wrapping_add((b as u32) << 8);
        let z = (len as u32).wrapping_add((c as u32) << 2);
        return shift_mix((y as u64).wrapping_mul(K2) ^ (z as u64).wrapping_mul(K0))
            .wrapping_mul(K2);
    }
    K2
}

// This probably works well for 16-byte strings as well, but it may be overkill
// in that case.
fn hash_len17_to_32(s: &[u8]) -> u64 {
    let len = s.len();
    let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
    let a = fetch64(s, 0).wrapping_mul(K1);
    let b = fetch64(s, 8);
    let c = fetch64(s, len - 8).wrapping_mul(mul);
    let d = fetch64(s, len - 16).wrapping_mul(K2);
    hash_len16_mul(
        rotate(a.wrapping_add(b), 43).wrapping_add(rotate(c, 30)).wrapping_add(d),
        a.wrapping_add(rotate(b.wrapping_add(K2), 18)).wrapping_add(c),
        mul,
    )
}

/// Returns a 16-byte hash for 48 bytes given as words. Quick and dirty.
/// Callers do best to use "random-looking" values for `a` and `b`.
#[inline]
fn weak_hash_len32_with_seeds_words(
    w: u64,
    x: u64,
    y: u64,
    z: u64,
    mut a: u64,
    mut b: u64,
) -> (u64, u64) {
    a = a.wrapping_add(w);
    b = rotate(b.wrapping_add(a).wrapping_add(z), 21);
    let c = a;
    a = a.wrapping_add(x);
    a = a.wrapping_add(y);
    b = b.wrapping_add(rotate(a, 44));
    (a.wrapping_add(z), b.wrapping_add(c))
}

/// Returns a 16-byte hash for `s[pos..pos+32]`, `a`, and `b`.
#[inline]
fn weak_hash_len32_with_seeds(s: &[u8], pos: usize, a: u64, b: u64) -> (u64, u64) {
    weak_hash_len32_with_seeds_words(
        fetch64(s, pos),
        fetch64(s, pos + 8),
        fetch64(s, pos + 16),
        fetch64(s, pos + 24),
        a,
        b,
    )
}

// Returns an 8-byte hash for 33 to 64 bytes.
fn hash_len33_to_64(s: &[u8]) -> u64 {
    let len = s.len();
    let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
    let mut a = fetch64(s, 0).wrapping_mul(K2);
    let b = fetch64(s, 8);
    let c = fetch64(s, len - 24);
    let d = fetch64(s, len - 32);
    let e = fetch64(s, 16).wrapping_mul(K2);
    let f = fetch64(s, 24).wrapping_mul(9);
    let g = fetch64(s, len - 8);
    let h = fetch64(s, len - 16).wrapping_mul(mul);
    let u =
        rotate(a.wrapping_add(g), 43).wrapping_add(rotate(b, 30).wrapping_add(c).wrapping_mul(9));
    let v = ((a.wrapping_add(g)) ^ d).wrapping_add(f).wrapping_add(1);
    let w = (u.wrapping_add(v).wrapping_mul(mul)).swap_bytes().wrapping_add(h);
    let x = rotate(e.wrapping_add(f), 42).wrapping_add(c);
    let y = (v.wrapping_add(w).wrapping_mul(mul).swap_bytes().wrapping_add(g)).wrapping_mul(mul);
    let z = e.wrapping_add(f).wrapping_add(c);
    a = (x.wrapping_add(z).wrapping_mul(mul).wrapping_add(y)).swap_bytes().wrapping_add(b);
    shift_mix(z.wrapping_add(a).wrapping_mul(mul).wrapping_add(d).wrapping_add(h))
        .wrapping_mul(mul)
        .wrapping_add(x)
}

/// 64-bit hash of a byte array (`CityHash64`).
pub fn cityhash64(s: &[u8]) -> u64 {
    let len = s.len();
    if len <= 32 {
        if len <= 16 {
            return hash_len0_to_16(s);
        } else {
            return hash_len17_to_32(s);
        }
    } else if len <= 64 {
        return hash_len33_to_64(s);
    }

    // For strings over 64 bytes we hash the end first, and then as we
    // loop we keep 56 bytes of state: v, w, x, y, and z.
    let mut x = fetch64(s, len - 40);
    let mut y = fetch64(s, len - 16).wrapping_add(fetch64(s, len - 56));
    let mut z = hash_len16(fetch64(s, len - 48).wrapping_add(len as u64), fetch64(s, len - 24));
    let mut v = weak_hash_len32_with_seeds(s, len - 64, len as u64, z);
    let mut w = weak_hash_len32_with_seeds(s, len - 32, y.wrapping_add(K1), x);
    x = x.wrapping_mul(K1).wrapping_add(fetch64(s, 0));

    // Decrease len to the nearest multiple of 64, and operate on 64-byte chunks.
    let mut rem = (len - 1) & !63usize;
    let mut pos = 0usize;
    loop {
        x = rotate(x.wrapping_add(y).wrapping_add(v.0).wrapping_add(fetch64(s, pos + 8)), 37)
            .wrapping_mul(K1);
        y = rotate(y.wrapping_add(v.1).wrapping_add(fetch64(s, pos + 48)), 42).wrapping_mul(K1);
        x ^= w.1;
        y = y.wrapping_add(v.0).wrapping_add(fetch64(s, pos + 40));
        z = rotate(z.wrapping_add(w.0), 33).wrapping_mul(K1);
        v = weak_hash_len32_with_seeds(s, pos, v.1.wrapping_mul(K1), x.wrapping_add(w.0));
        w = weak_hash_len32_with_seeds(
            s,
            pos + 32,
            z.wrapping_add(w.1),
            y.wrapping_add(fetch64(s, pos + 16)),
        );
        std::mem::swap(&mut z, &mut x);
        pos += 64;
        rem -= 64;
        if rem == 0 {
            break;
        }
    }
    hash_len16(
        hash_len16(v.0, w.0).wrapping_add(shift_mix(y).wrapping_mul(K1)).wrapping_add(z),
        hash_len16(v.1, w.1).wrapping_add(x),
    )
}

/// 64-bit hash of a byte array with a 64-bit seed also hashed in
/// (`CityHash64WithSeed`).
pub fn cityhash64_with_seed(s: &[u8], seed: u64) -> u64 {
    cityhash64_with_seeds(s, K2, seed)
}

/// 64-bit hash of a byte array with two 64-bit seeds also hashed in
/// (`CityHash64WithSeeds`).
pub fn cityhash64_with_seeds(s: &[u8], seed0: u64, seed1: u64) -> u64 {
    hash_len16(cityhash64(s).wrapping_sub(seed0), seed1)
}

// A subroutine for CityHash128(). Returns a decent 128-bit hash for strings
// of any length. Based on City and Murmur.
fn city_murmur(s: &[u8], seed: Uint128) -> Uint128 {
    let len = s.len();
    let mut a = uint128_low64(seed);
    let mut b = uint128_high64(seed);
    let mut c: u64;
    let d: u64;
    if len <= 16 {
        a = shift_mix(a.wrapping_mul(K1)).wrapping_mul(K1);
        c = b.wrapping_mul(K1).wrapping_add(hash_len0_to_16(s));
        d = shift_mix(a.wrapping_add(if len >= 8 { fetch64(s, 0) } else { c }));
    } else {
        c = hash_len16(fetch64(s, len - 8).wrapping_add(K1), a);
        let mut dd = hash_len16(b.wrapping_add(len as u64), c.wrapping_add(fetch64(s, len - 16)));
        a = a.wrapping_add(dd);
        // len > 16 here, so do...while is safe.
        let mut pos = 0usize;
        let mut rem = len;
        loop {
            a ^= shift_mix(fetch64(s, pos).wrapping_mul(K1)).wrapping_mul(K1);
            a = a.wrapping_mul(K1);
            b ^= a;
            c ^= shift_mix(fetch64(s, pos + 8).wrapping_mul(K1)).wrapping_mul(K1);
            c = c.wrapping_mul(K1);
            dd ^= c;
            pos += 16;
            rem -= 16;
            if rem <= 16 {
                break;
            }
        }
        d = dd;
    }
    a = hash_len16(a, c);
    b = hash_len16(d, b);
    (a ^ b, hash_len16(b, a))
}

/// 128-bit hash of a byte array with a 128-bit seed also hashed in
/// (`CityHash128WithSeed`).
pub fn cityhash128_with_seed(s: &[u8], seed: Uint128) -> Uint128 {
    let len = s.len();
    if len < 128 {
        return city_murmur(s, seed);
    }

    // We expect len >= 128 to be the common case. Keep 56 bytes of state:
    // v, w, x, y, and z.
    let mut x = uint128_low64(seed);
    let mut y = uint128_high64(seed);
    let mut z = (len as u64).wrapping_mul(K1);
    let v0 = rotate(y ^ K1, 49).wrapping_mul(K1).wrapping_add(fetch64(s, 0));
    let v1 = rotate(v0, 42).wrapping_mul(K1).wrapping_add(fetch64(s, 8));
    let mut v = (v0, v1);
    let mut w = (
        rotate(y.wrapping_add(z), 35).wrapping_mul(K1).wrapping_add(x),
        rotate(x.wrapping_add(fetch64(s, 88)), 53).wrapping_mul(K1),
    );

    // This is the same inner loop as CityHash64(), manually unrolled.
    let mut pos = 0usize;
    let mut rem = len;
    loop {
        for _ in 0..2 {
            x = rotate(x.wrapping_add(y).wrapping_add(v.0).wrapping_add(fetch64(s, pos + 8)), 37)
                .wrapping_mul(K1);
            y = rotate(y.wrapping_add(v.1).wrapping_add(fetch64(s, pos + 48)), 42).wrapping_mul(K1);
            x ^= w.1;
            y = y.wrapping_add(v.0).wrapping_add(fetch64(s, pos + 40));
            z = rotate(z.wrapping_add(w.0), 33).wrapping_mul(K1);
            v = weak_hash_len32_with_seeds(s, pos, v.1.wrapping_mul(K1), x.wrapping_add(w.0));
            w = weak_hash_len32_with_seeds(
                s,
                pos + 32,
                z.wrapping_add(w.1),
                y.wrapping_add(fetch64(s, pos + 16)),
            );
            std::mem::swap(&mut z, &mut x);
            pos += 64;
        }
        rem -= 128;
        if rem < 128 {
            break;
        }
    }

    x = x.wrapping_add(rotate(v.0.wrapping_add(z), 49).wrapping_mul(K0));
    y = y.wrapping_mul(K0).wrapping_add(rotate(w.1, 37));
    z = z.wrapping_mul(K0).wrapping_add(rotate(w.0, 27));
    w.0 = w.0.wrapping_mul(9);
    v.0 = v.0.wrapping_mul(K0);
    // If 0 < len < 128, hash up to 4 chunks of 32 bytes each from the end of s.
    // Absolute indices into `s` are used: `pos + rem` is always `s.len()`.
    let end = pos + rem;
    let mut tail_done = 0usize;
    while tail_done < rem {
        tail_done += 32;
        y = rotate(x.wrapping_add(y), 42).wrapping_mul(K0).wrapping_add(v.1);
        w.0 = w.0.wrapping_add(fetch64(s, end + 16 - tail_done));
        x = x.wrapping_mul(K0).wrapping_add(w.0);
        z = z.wrapping_add(w.1).wrapping_add(fetch64(s, end - tail_done));
        w.1 = w.1.wrapping_add(v.0);
        v = weak_hash_len32_with_seeds(s, end - tail_done, v.0.wrapping_add(z), v.1);
        v.0 = v.0.wrapping_mul(K0);
    }
    // At this point our 56 bytes of state should contain more than
    // enough information for a strong 128-bit hash. We use two
    // different 56-byte-to-8-byte hashes to get a 16-byte final result.
    x = hash_len16(x, v.0);
    y = hash_len16(y.wrapping_add(z), w.0);
    (
        hash_len16(x.wrapping_add(v.1), w.1).wrapping_add(y),
        hash_len16(x.wrapping_add(w.1), y.wrapping_add(v.1)),
    )
}

/// 128-bit hash of a byte array (`CityHash128`).
pub fn cityhash128(s: &[u8]) -> Uint128 {
    if s.len() >= 16 {
        cityhash128_with_seed(&s[16..], (fetch64(s, 0), fetch64(s, 8).wrapping_add(K0)))
    } else {
        cityhash128_with_seed(s, (K0, K1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple xorshift64 PRNG for deterministic inputs. No external crate.
    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    #[test]
    fn uint128_accessors() {
        let x: Uint128 = (7, 9);
        assert_eq!(uint128_low64(x), 7);
        assert_eq!(uint128_high64(x), 9);
    }

    #[test]
    fn empty_input_values() {
        // CityHash64 of the empty string is K2 by definition.
        assert_eq!(cityhash64(b""), K2);
        assert_eq!(cityhash64(b""), 0x9ae16a3b2f90404f);
    }

    #[test]
    fn seeded_variants_match_definitions() {
        let mut rng = XorShift(0x1234_5678_9abc_def1);
        for len in [0usize, 1, 3, 4, 7, 8, 15, 16, 17, 31, 32, 33, 64, 65, 200] {
            let data: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            let seed = rng.next();
            // CityHash64WithSeed(s, seed) == CityHash64WithSeeds(s, k2, seed).
            assert_eq!(cityhash64_with_seed(&data, seed), cityhash64_with_seeds(&data, K2, seed));
            // CityHash64WithSeeds definition.
            assert_eq!(
                cityhash64_with_seeds(&data, seed, seed ^ 1),
                hash128_to_64((cityhash64(&data).wrapping_sub(seed), seed ^ 1))
            );
        }
    }

    #[test]
    fn cityhash128_uses_seed_for_short_inputs() {
        // For len < 16, CityHash128 is CityHash128WithSeed with (k0, k1).
        for len in 0..16usize {
            let data: Vec<u8> = (0..len as u8).collect();
            assert_eq!(cityhash128(&data), cityhash128_with_seed(&data, (K0, K1)));
        }
        // For len >= 16, the first 16 bytes are the seed material.
        let data: Vec<u8> = (0..300u32).map(|i| (i * 7) as u8).collect();
        let seed = (fetch64(&data, 0), fetch64(&data, 8).wrapping_add(K0));
        assert_eq!(cityhash128(&data), cityhash128_with_seed(&data[16..], seed));
    }

    #[test]
    fn hash128_to_64_matches_formula() {
        // Hash128to64 follows the formula in city.h exactly.
        let x: Uint128 = (0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210);
        let kmul = 0x9ddfea08eb382d69u64;
        let mut a = (x.0 ^ x.1).wrapping_mul(kmul);
        a ^= a >> 47;
        let mut b = (x.1 ^ a).wrapping_mul(kmul);
        b ^= b >> 47;
        b = b.wrapping_mul(kmul);
        assert_eq!(hash128_to_64(x), b);
    }

    #[test]
    fn all_lengths_run_without_panic_and_are_deterministic() {
        let mut rng = XorShift(0xdead_beef_cafe_f00d);
        let data: Vec<u8> = (0..2100).map(|_| rng.next() as u8).collect();
        for len in 0..=2100usize {
            let s = &data[..len];
            let h1 = cityhash64(s);
            let h2 = cityhash64(s);
            assert_eq!(h1, h2);
            assert_eq!(cityhash128(s), cityhash128(s));
            assert_eq!(cityhash32(s), cityhash32(s));
        }
    }

    #[test]
    fn small_inputs_differ_across_lengths() {
        // Sanity check: inputs that differ only in length should hash differently.
        let data = [0u8; 300];
        let mut seen = std::collections::HashSet::new();
        for len in 0..=300usize {
            assert!(seen.insert(cityhash64(&data[..len])), "collision at {len}");
        }
    }

    #[test]
    fn single_bit_flips_change_output() {
        let mut rng = XorShift(42);
        for len in [1usize, 5, 13, 20, 40, 70, 130, 200] {
            let base: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            for i in 0..len {
                let mut flipped = base.clone();
                flipped[i] ^= 1 << (rng.next() % 8);
                assert_ne!(cityhash64(&base), cityhash64(&flipped), "len {len} idx {i}");
                assert_ne!(cityhash128(&base), cityhash128(&flipped), "len {len} idx {i}");
            }
        }
    }
}
