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

uint64_t mul32(uint a, uint b) {
  uint lo, hi;
  umulExtended(a, b, lo, hi);
  return uint64_t(hi) << 32 | uint64_t(lo);
}

uint128_t mul64(uint64_t a, uint64_t b) {
    uint   a_low = uint(a & 0xFFFFFFFFu);
    uint   a_high = uint(a >> 32);
    uint   b_low = uint(b & 0xFFFFFFFFu);
    uint   b_high = uint(b >> 32);

    uint64_t ll = mul32(a_low, b_low);
    uint64_t lh = mul32(a_low, b_high);
    uint64_t hl = mul32(a_high, b_low);
    uint64_t hh = mul32(a_high, b_high);

    // Start with ll in low part, hh in high part
    uint64_t result_lo = ll;
    uint64_t result_hi = hh;

    // Add lh shifted by 32 bits
    result_lo += lh << 32;
    uint carry1 = result_lo < (lh << 32) ? 1u : 0u;
    result_hi += (lh >> 32) + carry1;

    //Add hl shifted by 32 bits
    uint64_t old_lo = result_lo;
    result_lo += hl << 32;
    uint carry2 = result_lo < old_lo ? 1u : 0u;
    result_hi += (hl >> 32) + carry2;

    return uint128_t(result_lo, result_hi);
}

uint64_t montReduction(uint128_t x) {
    uint64_t x1 = x.lo;
    uint64_t x2 = x.hi;

    uint64_t a = x1 + (x1 << 32);
    uint carry1 = a < (x1 << 32) ? 1u : 0u;
    uint64_t b = a - (a >> 32) - uint64_t(carry1); // TODO: helper functions for wrapping_sub/add 

    uint64_t r = x2 - b;
    uint borrow = x2 < b ? 1u : 0u;

    const uint64_t PRIME = uint64_t(0xFFFFFFFFu) << 32 | uint64_t(1u);
    uint64_t result = r - ((1 + ~PRIME) * uint64_t(borrow));

    return result;
}

// x and y is in montgommery form?
// melt = montgommery form
// belt = regular form
// mont reduction is faster than regular mod operator
// (x * y) % p
uint64_t montiply(uint64_t x, uint64_t y) {
    return montReduction(mul64(x, y));
}
