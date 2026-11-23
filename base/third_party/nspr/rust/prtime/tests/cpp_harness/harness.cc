// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Reads lines "<hex-input> <default_to_gmt 0|1>" from stdin and prints
// "<ok 0|1> <result>" using the unmodified C++ PR_ParseTimeString. It
// produced the rows in tests/cpp_golden.rs. The shim/ directory holds
// stand-in headers (DCHECK as a no-op, BUILDFLAG as 0). Run with TZ=UTC for
// offset 0 and TZ=PST+8 for offset -28800. Build from the repository root:
//   g++ -std=c++17 -O1 -Ibase/third_party/nspr/rust/prtime/tests/cpp_harness/shim -I. \
//     base/third_party/nspr/rust/prtime/tests/cpp_harness/harness.cc \
//     base/third_party/nspr/prtime.cc -o /tmp/prtime_harness
#include <cstdio>
#include <cstdint>
#include <string>
#include <iostream>
#include "base/third_party/nspr/prtime.h"
static int hexval(char c) { return c <= '9' ? c - '0' : (c | 32) - 'a' + 10; }
int main() {
  std::string line;
  while (std::getline(std::cin, line)) {
    size_t sp = line.find(' ');
    std::string hex = line.substr(0, sp);
    int gmt = line[sp + 1] == '1';
    std::string bytes;
    for (size_t i = 0; i + 1 < hex.size() + 1 && i < hex.size(); i += 2)
      bytes.push_back((char)(hexval(hex[i]) * 16 + hexval(hex[i + 1])));
    PRTime r = 0;
    PRStatus st = PR_ParseTimeString(bytes.c_str(), gmt, &r);
    printf("%d %lld\n", st == PR_SUCCESS ? 1 : 0, (long long)(st == PR_SUCCESS ? r : 0));
  }
  return 0;
}
