// Shim: DCHECK_EQ is a no-op for the golden harness.
#ifndef SHIM_BASE_CHECK_OP_H_
#define SHIM_BASE_CHECK_OP_H_
#define DCHECK_EQ(a, b) ((void)0)
#endif
