// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Test-only helpers: gtest-style float comparison and a small PRNG.

/// Maps a float bit pattern to an integer that increases with the float value,
/// so the distance between two of these integers counts representable floats.
/// This is the same mapping gtest uses in `FloatingPoint::AlmostEquals`.
fn biased_f32(bits: u32) -> u32 {
    if bits & 0x8000_0000 != 0 {
        (!bits).wrapping_add(1)
    } else {
        bits | 0x8000_0000
    }
}

fn biased_f64(bits: u64) -> u64 {
    if bits & 0x8000_0000_0000_0000 != 0 {
        (!bits).wrapping_add(1)
    } else {
        bits | 0x8000_0000_0000_0000
    }
}

/// `EXPECT_FLOAT_EQ`: equal within 4 units in the last place. NaN never
/// compares equal, and +0 equals -0.
pub fn float_eq(a: f32, b: f32) -> bool {
    if a.is_nan() || b.is_nan() {
        return false;
    }
    biased_f32(a.to_bits()).abs_diff(biased_f32(b.to_bits())) <= 4
}

/// `EXPECT_DOUBLE_EQ`: equal within 4 units in the last place.
pub fn double_eq(a: f64, b: f64) -> bool {
    if a.is_nan() || b.is_nan() {
        return false;
    }
    biased_f64(a.to_bits()).abs_diff(biased_f64(b.to_bits())) <= 4
}

/// `xorshift64` pseudo-random generator. Deterministic, with no external
/// crates.
pub struct XorShift64(u64);

impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        // A zero state would stay zero forever.
        XorShift64(if seed == 0 { 0x2545_f491_4f6c_dd1d } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `[0, 1)`.
    pub fn next_unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform `i32` over the full range.
    pub fn next_i32(&mut self) -> i32 {
        self.next_u64() as u32 as i32
    }

    /// Uniform finite `f32` in `[lo, hi)`.
    pub fn next_f32_finite_range(&mut self, lo: f32, hi: f32) -> f32 {
        (lo as f64 + self.next_unit() * (hi as f64 - lo as f64)) as f32
    }

    /// Uniform finite `f64` in `[lo, hi)`.
    pub fn next_f64_finite_range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.next_unit() * (hi - lo)
    }
}
