#define PELT Felt
#define GET_PELT(x, i) Felt(x[i * 3], x[i * 3 + 1], x[i * 3 + 2])
#define SET_PELT(o, i, v) { PELT v2 = v; o[i * 3] = v2.x; o[i * 3 + 1] = v2.y; o[i * 3 + 2] = v2.z; }
#define PMUL(a, b) fmul(a, b)
#define PADD(a, b) fadd(a, b)
#define PSUB(a, b) fsub(a, b)
#define PRINT_PELT(f, v) debugPrintfEXT("%08x%08x %08x%08x %08x%08x", uint(v.x >> 32), uint(v.x), uint(v.y >> 32), uint(v.y), uint(v.z >> 32), uint(v.z))
#include <p_ntt_base>
