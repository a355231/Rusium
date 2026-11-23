// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Test-only helpers: a small xorshift PRNG, so the crate needs no `rand`.

/// xorshift64 generator. Deterministic for a given seed.
pub(crate) struct XorShift64(u64);

impl XorShift64 {
    pub(crate) fn new(seed: u64) -> Self {
        // xorshift must not be seeded with zero.
        Self(seed.max(1))
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub(crate) fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    pub(crate) fn next_u16(&mut self) -> u16 {
        (self.next_u64() >> 48) as u16
    }

    pub(crate) fn next_u8(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }

    /// Returns a value in `0..n`. `n` must be non-zero.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}
