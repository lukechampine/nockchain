#if defined(EMULATE_EXTENDED_MATH) && (EMULATE_EXTENDED_MATH > 0)
#include <math_emu>
#endif

struct ReduceOp {
    uint source;
    uint destination;
};

struct uint128_t {
    uint64_t lo;
    uint64_t hi;
};

void umul32(uint a, uint b, out uint clo, out uint chi) {
    umulExtended(a, b, clo, chi);//clo = a * b;
}

uint128_t mul64(uint64_t x, uint64_t y)
{
    // Split each operand into 32-bit halves
    uint   x_lo = uint(x);
    uint   x_hi = uint(x >> 32);
    uint   y_lo = uint(y);
    uint   y_hi = uint(y >> 32);

    // p0 = x_lo * y_lo
    uint p0_lo, p0_hi;
    umul32(x_lo, y_lo, p0_lo, p0_hi);
    // cross terms: x_lo*y_hi and x_hi*y_lo
    uint c1_lo, c1_hi;
    umul32(x_lo, y_hi, c1_lo, c1_hi);
    uint c2_lo, c2_hi;
    umul32(x_hi, y_lo, c2_lo, c2_hi);

    // reconstruct the full 64-bit p0
    uint64_t p0 = (uint64_t(p0_hi) << 32) | uint64_t(p0_lo);

    // sum the “middle” 64-bit pieces:
    //    (upper 32 of p0) + c1_lo + c2_lo
    uint64_t mid = (p0 >> 32)
                 + uint64_t(c1_lo)
                 + uint64_t(c2_lo);

    // low 64 bits of the final product
    uint64_t lo = (uint64_t(uint(p0))        )   // low 32 of p0
       | (uint64_t(uint(mid)) << 32);    // low 32 of mid

    // high 64 bits of the final product:
    //   (x_hi * y_hi)      -- 32×32→64
    // +        c1_hi      -- carry from x_lo*y_hi
    // +        c2_hi      -- carry from x_hi*y_lo
    // + (mid >> 32)       -- carry from the middle sum
    uint64_t hi = uint64_t(x_hi) * uint64_t(y_hi)
       + uint64_t(c1_hi)
       + uint64_t(c2_hi)
       + (mid >> 32);

    return uint128_t(lo, hi);
}

uint64_t montReduction(uint128_t x) {
    uint64_t x1 = x.lo;
    uint64_t x2 = x.hi;

    uint64_t a = x1 + (x1 << 32);
    bool e = a < x1;
    uint64_t b = (a - (a >> 32)) - uint64_t(e);

    uint64_t r = x2 - b;
    bool c = r > x2;

    return r - uint64_t(0xffff * uint(c));
}

uint64_t montiply(uint64_t x, uint64_t y) {
    return montReduction(mul64(x, y));
}
