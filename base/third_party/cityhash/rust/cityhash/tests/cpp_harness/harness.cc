// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Generator for tests/cpp_golden.rs. It runs the unmodified
// base/third_party/cityhash/city.cc and prints one table row per input.
// Build and run from the repository root:
//   g++ -std=c++17 -O1 -Ibase/third_party/cityhash -I. \
//     base/third_party/cityhash/rust/cityhash/tests/cpp_harness/harness.cc \
//     base/third_party/cityhash/city.cc -o /tmp/city_harness && /tmp/city_harness
// Golden-vector generator for CityHash (unmodified city.cc from the repo).
#include <cstdint>
#include <cstdio>
#include <vector>
#include "city.h"

using base::internal::cityhash_v111::uint128;
using base::internal::cityhash_v111::CityHash64;
using base::internal::cityhash_v111::CityHash64WithSeed;
using base::internal::cityhash_v111::CityHash64WithSeeds;
using base::internal::cityhash_v111::CityHash128;
using base::internal::cityhash_v111::CityHash128WithSeed;
using base::internal::cityhash_v111::CityHash32;
using base::internal::cityhash_v111::Uint128Low64;
using base::internal::cityhash_v111::Uint128High64;

struct Rng {
  uint64_t s;
  uint64_t next() {
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    return s;
  }
};

// kind 0: pattern(len); kind 1: xorshift bytes seeded by len.
static std::vector<char> MakeBuf(int kind, size_t len) {
  std::vector<char> b(len);
  if (kind == 0) {
    for (size_t i = 0; i < len; i++)
      b[i] = (char)(uint8_t)((i * 167 + len * 13 + 89) % 256);
  } else {
    Rng r{(uint64_t)len * 0x9E3779B97F4A7C15ULL | 1};
    for (size_t i = 0; i < len; i++) b[i] = (char)(uint8_t)(r.next() >> 24);
  }
  return b;
}

static void Emit(int kind, size_t len, uint64_t seed64, uint64_t s0,
                  uint64_t s1, uint128 seed128) {
  std::vector<char> b = MakeBuf(kind, len);
  const char* p = b.data();
  uint128 h128 = CityHash128(p, len);
  uint128 h128s = CityHash128WithSeed(p, len, seed128);
  printf("    (%d, %zu, [0x%llx, 0x%llx, 0x%llx, 0x%llx, 0x%llx], [0x%llx, 0x%llx, 0x%llx, 0x%llx, 0x%llx, 0x%llx, 0x%llx, 0x%llx]),\n",
         kind, len, (unsigned long long)seed64, (unsigned long long)s0,
         (unsigned long long)s1, (unsigned long long)Uint128Low64(seed128),
         (unsigned long long)Uint128High64(seed128),
         (unsigned long long)CityHash64(p, len),
         (unsigned long long)CityHash64WithSeed(p, len, seed64),
         (unsigned long long)CityHash64WithSeeds(p, len, s0, s1),
         (unsigned long long)Uint128Low64(h128),
         (unsigned long long)Uint128High64(h128),
         (unsigned long long)Uint128Low64(h128s),
         (unsigned long long)Uint128High64(h128s),
         (unsigned long long)CityHash32(p, len));
}

int main() {
  // Random seeds per (kind, len).
  for (int kind = 0; kind < 2; kind++) {
    for (size_t len = 0; len <= 600; len++) {
      Rng sr{(0xD1B54A32D192ED03ULL ^ ((uint64_t)len * 0xA24BAED4963EE407ULL) ^ ((uint64_t)kind << 40)) | 1};
      uint64_t seed64 = sr.next();
      uint64_t s0 = sr.next();
      uint64_t s1 = sr.next();
      uint64_t lo = sr.next();
      uint64_t hi = sr.next();
      Emit(kind, len, seed64, s0, s1, uint128(lo, hi));
    }
  }
  // Special seeds (0 and all-ones) at boundary lengths.
  static const size_t kSpecial[] = {0, 1, 2, 3, 4, 5, 8, 12, 13, 16, 17, 24, 25,
                                    31, 32, 33, 48, 63, 64, 65, 96, 127, 128,
                                    129, 191, 192, 200, 255, 256, 257, 300,
                                    511, 512, 513, 600};
  for (int kind = 0; kind < 2; kind++) {
    for (size_t len : kSpecial) {
      Emit(kind, len, 0, 0, 0, uint128(0, 0));
      Emit(kind, len, ~0ULL, ~0ULL, ~0ULL, uint128(~0ULL, ~0ULL));
    }
  }
  return 0;
}
