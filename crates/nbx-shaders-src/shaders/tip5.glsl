#include <generated/tip5>
#include <base>

// NOTE: we need to find optimal size for the sponge operations,
// in order to have the right balance between parallelism and VGPR usage.
#define SP64SZ 1

#if SP64SZ == 4
#define SPONGE(_0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15) Sponge(u64vec4[tip5StateSize / 4]( \
    u64vec4(_0, _1, _2, _3), \
    u64vec4(_4, _5, _6, _7), \
    u64vec4(_8, _9, _10, _11), \
    u64vec4(_12, _13, _14, _15) \
))
#define SP64 u64vec4
#elif SP64SZ == 2
#define SPONGE(_0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15) Sponge(u64vec2[tip5StateSize / 2]( \
    u64vec2(_0, _1), \
    u64vec2(_2, _3), \
    u64vec2(_4, _5), \
    u64vec2(_6, _7), \
    u64vec2(_8, _9), \
    u64vec2(_10, _11), \
    u64vec2(_12, _13), \
    u64vec2(_14, _15) \
))
#define SP64 u64vec2
#elif SP64SZ == 1
#define SPONGE(_0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15) Sponge(uint64_t[tip5StateSize](_0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15))
#define SP64 uint64_t
#endif

struct Sponge {
    SP64 s[tip5StateSize / SP64SZ];
};

void spongeSet(in out Sponge state, uint idx, uint64_t val) {
#if SP64SZ == 1
    state.s[idx] = val;
#else
    state.s[idx / SP64SZ][idx % SP64SZ] = val;
#endif
}

uint64_t spongeGet(Sponge state, uint idx) {
#if SP64SZ == 1
    return state.s[idx];
#else
    return state.s[idx / SP64SZ][idx % SP64SZ];
#endif
}

#include <generated/sponge>

#if defined(HAS_PRINTF_EXT)
void tip5SpongePrint(Sponge sp) {
    debugPrintfEXT("%08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x",
        uint(spongeGet(sp, 0) >> 32), uint(spongeGet(sp, 0)),
        uint(spongeGet(sp, 1) >> 32), uint(spongeGet(sp, 1)),
        uint(spongeGet(sp, 2) >> 32), uint(spongeGet(sp, 2)),
        uint(spongeGet(sp, 3) >> 32), uint(spongeGet(sp, 3)),
        uint(spongeGet(sp, 4) >> 32), uint(spongeGet(sp, 4)),
        uint(spongeGet(sp, 5) >> 32), uint(spongeGet(sp, 5)),
        uint(spongeGet(sp, 6) >> 32), uint(spongeGet(sp, 6)),
        uint(spongeGet(sp, 7) >> 32), uint(spongeGet(sp, 7)),
        uint(spongeGet(sp, 8) >> 32), uint(spongeGet(sp, 8)),
        uint(spongeGet(sp, 9) >> 32), uint(spongeGet(sp, 9)),
        uint(spongeGet(sp, 10) >> 32), uint(spongeGet(sp, 10)),
        uint(spongeGet(sp, 11) >> 32), uint(spongeGet(sp, 11)),
        uint(spongeGet(sp, 12) >> 32), uint(spongeGet(sp, 12)),
        uint(spongeGet(sp, 13) >> 32), uint(spongeGet(sp, 13)),
        uint(spongeGet(sp, 14) >> 32), uint(spongeGet(sp, 14)),
        uint(spongeGet(sp, 15) >> 32), uint(spongeGet(sp, 15))
    );
}
#else
void tip5SpongePrint(Sponge sp) {
}
#endif


SP64 tip5SboxOne(SP64 vIn) {
#if SP64SZ == 1
    uint64_t v = vIn;
#else
    SP64 ret;
    for (uint o = 0; o < SP64SZ; o += 1) {
        uint64_t v = vIn[o];
#endif
        uint64_t r = 0;
        for (uint i = 0; i < 8; i += 1) {
            uint i1 = uint(v & 0xff);
            uint64_t tmp = tip5LookupTable[uint(i1)];
            v >>= 8;
            r |= tmp << (i * 8);
        }
#if SP64SZ == 1
        return r;
#else
        ret[o] = r;
    }
    return ret;
#endif
}

Sponge tip5SboxLayer(Sponge state) {
    Sponge res;

    for (uint i = 0; i < 4 / SP64SZ; i += 1) {
        SP64 v = tip5SboxOne(state.s[i]);
        res.s[i] = v;
    }

    for (uint j = 4 / SP64SZ; j < tip5StateSize / SP64SZ; j += 1) {
        res.s[j] = mpow(state.s[j], 7, 3);
    }

    return res;
}

uint64_t tip5LinearIter(Sponge state, uint i, uint j) {
    u64vec4 me1 = tip5MdsMatrixVec[i][j * 2];
    u64vec4 me2 = tip5MdsMatrixVec[i][j * 2 + 1];

#if SP64SZ == 4
    SP64 s1 = state.s[j * 2];
    SP64 s2 = state.s[j * 2 + 1];

    SP64 p1 = montiply(me1, s1);
    SP64 p2 = montiply(me2, s2);

    SP64 r1 = badd(p1, p2);
    u64vec2 r2 = badd(r1.xy, r1.zw);
    uint64_t r = badd(r2.x, r2.y);
#elif SP64SZ == 2
    u64vec2 s1 = state.s[(j * 2) * 2];
    u64vec2 s2 = state.s[(j * 2 + 1) * 2];

    u64vec2 p1 = montiply(me1.xy, s1);
    u64vec2 p2 = montiply(me2.xy, s2);

    u64vec2 r1 = badd(p1, p2);

    s1 = state.s[(j * 2) * 2 + 1];
    s2 = state.s[(j * 2 + 1) * 2 + 1];

    p1 = montiply(me1.zw, s1);
    p2 = montiply(me2.zw, s2);

    u64vec2 r2 = badd(p1, p2);

    u64vec2 r3 = badd(r1, r2);
    uint64_t r = badd(r3.x, r3.y);
#elif SP64SZ == 1
    uint64_t r = zeroMelt;
    for (uint i = 0; i < 4; i += 1) {
        uint64_t s1 = state.s[(j * 2) * 4 + i];
        uint64_t s2 = state.s[(j * 2 + 1) * 4 + i];

        uint64_t p1 = montiply(me1[i], s1);
        uint64_t p2 = montiply(me2[i], s2);
        uint64_t p = badd(p1, p2);

        r = badd(r, p);
    }
#endif
    return r;
}

Sponge tip5LinearLayer(Sponge state) {
    Sponge res;

    for (uint i = 0; i < 16; i += 1) {
        uint64_t a = tip5LinearIter(state, i, 0);
        uint64_t b = tip5LinearIter(state, i, 1);
        uint64_t val = badd(a, b);
        spongeSet(res, i, val);
    }

    return res;
}

void tip5Permute(in out Sponge sponge) {
    for (uint i = 0; i < tip5NumRounds; i += 1) {
        Sponge a = tip5SboxLayer(sponge);
        Sponge b = tip5LinearLayer(a);

        for (uint j = 0; j < tip5StateSize / SP64SZ; j += 1) {
            u64vec4 rConsVec = tip5RoundConstantsVec[i * (tip5StateSize / 4) + j / (4 / SP64SZ)];
#if SP64SZ == 1
            uint64_t rCons = rConsVec[j % 4];
#elif SP64SZ == 2
            u64vec2 rCons = u64vec2(rConsVec[(j % 2) * 2], rConsVec[(j % 2) * 2 + 1]);
#else
            u64vec4 rCons = rConsVec;
#endif
            sponge.s[j] = badd(rCons, b.s[j]);
        }
    }
}
