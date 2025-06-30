#include <generated/tip5>
#include <base>

struct Sponge {
    uint64_t s[tip5StateSize];
};

#include <generated/sponge>

#if defined(HAS_PRINTF_EXT)
void tip5SpongePrint(Sponge sp) {
    debugPrintfEXT("%08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x %08x%08x",
        uint(sp.s[0] >> 32), uint(sp.s[0]),
        uint(sp.s[1] >> 32), uint(sp.s[1]),
        uint(sp.s[2] >> 32), uint(sp.s[2]),
        uint(sp.s[3] >> 32), uint(sp.s[3]),
        uint(sp.s[4] >> 32), uint(sp.s[4]),
        uint(sp.s[5] >> 32), uint(sp.s[5]),
        uint(sp.s[6] >> 32), uint(sp.s[6]),
        uint(sp.s[7] >> 32), uint(sp.s[7]),
        uint(sp.s[8] >> 32), uint(sp.s[8]),
        uint(sp.s[9] >> 32), uint(sp.s[9]),
        uint(sp.s[10] >> 32), uint(sp.s[10]),
        uint(sp.s[11] >> 32), uint(sp.s[11]),
        uint(sp.s[12] >> 32), uint(sp.s[12]),
        uint(sp.s[13] >> 32), uint(sp.s[13]),
        uint(sp.s[14] >> 32), uint(sp.s[14]),
        uint(sp.s[15] >> 32), uint(sp.s[15])
    );
}
#endif

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

    for (uint i = 0; i < tip5NumSplitAndLookup; i+= 1) {
        res.s[i] = tip5SboxOne(state.s[i]);
    }

    for (uint j = tip5NumSplitAndLookup; j < tip5StateSize; j += 1) {
        res.s[j] = mpow(state.s[j], 7);
    }

    return res;
}

Sponge tip5LinearLayer(Sponge state) {
    Sponge res;

    for (uint i = 0; i < 16; i += 1) {
        res.s[i] = 0;
        for (uint j = 0; j < 16; j += 1) {
            uint64_t matrixElement = tip5MdsMatrix[i][j];
            uint64_t product = montiply(matrixElement, state.s[j]);
            res.s[i] = badd(res.s[i], product);
        }
    }

    return res;
}

void tip5Permute(in out Sponge sponge) {
    for (uint i = 0; i < tip5NumRounds; i += 1) {
        Sponge a = tip5SboxLayer(sponge);
        Sponge b = tip5LinearLayer(a);

        for (uint j = 0; j < tip5StateSize; j += 1) {
            uint64_t rCons = tip5RoundConstants[i * tip5StateSize + j];
            sponge.s[j] = badd(rCons, b.s[j]);
        }
    }
}
