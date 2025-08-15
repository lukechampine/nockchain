#define PELT uint64_t
#define GET_PELT(x, i) x[i]
#define SET_PELT(o, i, v) o[i] = v
#define PMUL(a, b) bmul(a, b)
#define PADD(a, b) badd(a, b)
#define PSUB(a, b) bsub(a, b)
#define PRINT_PELT(f, v) {}
//debugPrintfEXT(f "%08x%08x %08x%08x %08x%08x", uint(v.x >> 32), uint(v.x), uint(v.y >> 32), uint(v.y), uint(v.z >> 32), uint(v.z))
#include <p_ntt_base>
