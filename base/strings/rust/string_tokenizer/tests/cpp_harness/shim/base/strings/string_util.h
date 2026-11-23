// Shim: only IsAsciiWhitespace (body copied verbatim from the real
// base/strings/string_util.h) and the real whitespace constants.
#ifndef SHIM_BASE_STRINGS_STRING_UTIL_H_
#define SHIM_BASE_STRINGS_STRING_UTIL_H_
#include <concepts>
#include "base/strings/whitespace_constants.h"
namespace base {
template <typename Char>
  requires(std::integral<Char>)
constexpr bool IsAsciiWhitespace(Char c) {
  for (const char* cur = kWhitespaceASCII; *cur; ++cur) {
    if (*cur == c) {
      return true;
    }
  }
  return false;
}
}  // namespace base
#endif
