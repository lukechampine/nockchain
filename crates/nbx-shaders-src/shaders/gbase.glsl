struct U128 {
    U64 lo;
    U64 hi;
};

void umul32(U32 a, U32 b, out U32 chi, out U32 clo) {
    umulExtended(a, b, chi, clo);//clo = a * b;
}

U64 mul32(U32 a, U32 b) {
  U32 lo, hi;
  umulExtended(a, b, hi, lo);
  return U64(hi) << 32 | U64(lo);
}

U128 mul64(U64 a, U64 b) {
    U32   a_low = U32(a & 0xFFFFFFFFu);
    U32   a_high = U32(a >> 32);
    U32   b_low = U32(b & 0xFFFFFFFFu);
    U32   b_high = U32(b >> 32);

    U64 ll = mul32(a_low, b_low);
    U64 lh = mul32(a_low, b_high);
    U64 hl = mul32(a_high, b_low);
    U64 hh = mul32(a_high, b_high);

    // Start with ll in low part, hh in high part
    U64 result_lo = ll;
    U64 result_hi = hh;

    // Add lh shifted by 32 bits
    result_lo += lh << 32;
    U32 carry1 = U32(lessThan(result_lo, lh << 32));
    result_hi += (lh >> 32) + carry1;

    //Add hl shifted by 32 bits
    U64 old_lo = result_lo;
    result_lo += hl << 32;
    U32 carry2 = U32(lessThan(result_lo, old_lo));
    result_hi += (hl >> 32) + carry2;

    return U128(result_lo, result_hi);
}

U64 montReduction(U128 x) {
    U64 x1 = x.lo;
    U64 x2 = x.hi;

    U64 a = x1 + (x1 << 32);
    U32 carry1 = U32(lessThan(a, (x1 << 32)));
    U64 b = a - (a >> 32) - U64(carry1); // TODO: helper functions for wrapping_sub/add

    U64 r = x2 - b;
    U32 borrow = U32(lessThan(x2, b));

    U64 result = r - ((1 + ~PRIME) * U64(borrow));

    return result;
}

// x and y is in montgommery form?
// melt = montgommery form
// belt = regular form
// mont reduction is faster than regular mod operator
// (x * y) % p
U64 montiply(U64 x, U64 y) {
    return montReduction(mul64(x, y));
}

U64 badd(U64 a, U64 b) {
    U64 c = PRIME - b;
    U64 x1 = a - c;
    return x1 + (PRIME * U64(lessThan(a, c)));
}

I32 findMSB64(U64 x) {
    U32 lo = U32(x);
    U32 hi = U32(x >> 32);
    I32 msbLo = findMSB(lo);
    I32 msbHi = findMSB(hi);
    return mix(msbLo, msbHi, notEqual(hi, U32(0)));
}

U32 leadingZeros64(U64 x) {
    // findMSB(x) returns:
    //  - the index [0..63] of the highest-set bit, or
    //  - −1 if x == 0
    I32 msb = findMSB64(x);
    // else count how many bits above the MSB are zero
    return U32(I32(63) - msb);
}

U64 mpow(U64 v, uint e) {
    U64 acc = U64(oneMelt);
    uint bitLength = 64 - leadingZeros64(e);
    for (uint i = 0; i < bitLength; i += 1) {
        acc = montiply(acc, acc);
        bool bitSet = (e & (1 << (bitLength - 1 - i))) != 0;
        uint64_t bv = uint64_t(bitSet);
        acc = montiply(acc, U64(1 - bv) + U64(bv) * v);
    }
    return acc;
}

#undef U64
#undef U32
#undef I32
#undef U128
#undef BOOL
