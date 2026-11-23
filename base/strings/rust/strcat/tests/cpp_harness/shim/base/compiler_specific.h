// Shim: annotation macros are identity or empty.
#ifndef SHIM_BASE_COMPILER_SPECIFIC_H_
#define SHIM_BASE_COMPILER_SPECIFIC_H_
#define UNSAFE_BUFFERS(...) __VA_ARGS__
#define UNSAFE_TODO(...) __VA_ARGS__
#endif
