// Shim: annotation macros reduced to no-ops or identity.
#ifndef SHIM_BASE_COMPILER_SPECIFIC_H_
#define SHIM_BASE_COMPILER_SPECIFIC_H_
#define LIFETIME_BOUND
#define UNSAFE_TODO(...) __VA_ARGS__
#define UNSAFE_BUFFERS(...) __VA_ARGS__
#endif
