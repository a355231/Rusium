// Shim: base::span is std::span in this harness.
#ifndef SHIM_BASE_CONTAINERS_SPAN_H_
#define SHIM_BASE_CONTAINERS_SPAN_H_
#include <algorithm>  // Transitive in Chromium base; needed by std::ranges::copy.
#include <span>
namespace base {
using std::span;
}
#endif
