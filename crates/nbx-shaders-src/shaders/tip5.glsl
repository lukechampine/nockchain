#include <generated/tip5>
#include <base>

struct Sponge {
    u64vec4 s[tip5StateSize / 4];
};

void spongeSet(in out Sponge state, uint idx, uint64_t val) {
    state.s[idx / 4][idx % 4] = val;
}

uint64_t spongeGet(Sponge state, uint idx) {
    return state.s[idx / 4][idx % 4];
}

#include <generated/sponge>

#if defined(HAS_PRINTF_EXT)
void tip5SpongePrint(Sponge sp) {
    debugPrintfEXT("%08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x",
        uint(sp.s[0][0] >> 32), uint(sp.s[0][0]),
        uint(sp.s[0][1] >> 32), uint(sp.s[0][1]),
        uint(sp.s[0][2] >> 32), uint(sp.s[0][2]),
        uint(sp.s[0][3] >> 32), uint(sp.s[0][3]),
        uint(sp.s[1][0] >> 32), uint(sp.s[1][0]),
        uint(sp.s[1][1] >> 32), uint(sp.s[1][1]),
        uint(sp.s[1][2] >> 32), uint(sp.s[1][2]),
        uint(sp.s[1][3] >> 32), uint(sp.s[1][3]),
        uint(sp.s[2][0] >> 32), uint(sp.s[2][0]),
        uint(sp.s[2][1] >> 32), uint(sp.s[2][1]),
        uint(sp.s[2][2] >> 32), uint(sp.s[2][2]),
        uint(sp.s[2][3] >> 32), uint(sp.s[2][3]),
        uint(sp.s[3][0] >> 32), uint(sp.s[3][0]),
        uint(sp.s[3][1] >> 32), uint(sp.s[3][1]),
        uint(sp.s[3][2] >> 32), uint(sp.s[3][2]),
        uint(sp.s[3][3] >> 32), uint(sp.s[3][3])
    );
}
#endif

u64vec4 tip5SboxOne(u64vec4 v) {
    u64vec4 ret = u64vec4(0);
    for (uint i = 0; i < 8; i += 1) {
        uint v1 = tip5LookupTable[uint(v.x & 0xff)];
        uint v2 = tip5LookupTable[uint(v.y & 0xff)];
        uint v3 = tip5LookupTable[uint(v.z & 0xff)];
        uint v4 = tip5LookupTable[uint(v.w & 0xff)];
        u64vec4 tmp = u64vec4(v1, v2, v3, v4);
        v = v >> 8;
        ret |= tmp << (i * 8);
    }
    return ret;
}

uint64_t tip5SboxOne(uint64_t v) {
    uint64_t ret = 0;
    for (uint i = 0; i < 8; i += 1) {
        uint64_t tmp = uint64_t(tip5LookupTable[uint(v & 0xff)]);
        v = v >> 8;
        ret |= tmp << (i * 8);
    }
    return ret;
}

Sponge tip5SboxLayer(Sponge state) {
    Sponge res;

    u64vec4 v = tip5SboxOne(state.s[0]);
    res.s[0] = v;

    for (uint j = 1; j < tip5StateSize / 4; j += 1) {
        res.s[j] = mpow(state.s[j], 7, 3);
    }

    return res;
}

uint64_t tip5LinearIter(Sponge state, uint i, uint j) {
    u64vec4 me1 = tip5MdsMatrixVec[i][j * 2];
    u64vec4 me2 = tip5MdsMatrixVec[i][j * 2 + 1];

    u64vec4 s1 = state.s[j * 2];
    u64vec4 s2 = state.s[j * 2 + 1];

    u64vec4 p1 = montiply(me1, s1);
    u64vec4 p2 = montiply(me2, s2);

    u64vec4 r1 = badd(p1, p2);
    u64vec2 r2 = badd(r1.xy, r1.zw);
    uint64_t r3 = badd(r2.x, r2.y);

    return r3;
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

        for (uint j = 0; j < tip5StateSize / 4; j += 1) {
            u64vec4 rCons = tip5RoundConstantsVec[i * (tip5StateSize / 4) + j];
            sponge.s[j] = badd(rCons, b.s[j]);
        }
    }
}
