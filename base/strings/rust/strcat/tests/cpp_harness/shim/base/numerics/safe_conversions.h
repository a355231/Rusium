// Shim: checked_cast without the range check (the inputs here are in range).
#ifndef SHIM_BASE_NUMERICS_SAFE_CONVERSIONS_H_
#define SHIM_BASE_NUMERICS_SAFE_CONVERSIONS_H_
namespace base {
template <typename Dst, typename Src>
constexpr Dst checked_cast(Src v) {
  return static_cast<Dst>(v);
}
}  // namespace base
#endif
