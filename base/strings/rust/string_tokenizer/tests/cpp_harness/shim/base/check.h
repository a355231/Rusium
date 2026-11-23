// Shim: DCHECK is a no-op for the golden harness.
#ifndef SHIM_BASE_CHECK_H_
#define SHIM_BASE_CHECK_H_
#define DCHECK(cond) ((void)0)
#endif
