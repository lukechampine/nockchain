void umulExtended(uint a, uint b, out uint lo, out uint hi) {
    uint a_lo = a & 0xFFFFu;
    uint a_hi = a >> 16;
    uint b_lo = b & 0xFFFFu;
    uint b_hi = b >> 16;

    uint p0 = a_lo * b_lo;
    uint p1 = a_lo * b_hi;
    uint p2 = a_hi * b_lo;
    uint p3 = a_hi * b_hi;

    uint mid = p1 + p2;
    uint carry_mid = mid < p1 ? 1u : 0u;

    lo = p0 + (mid << 16);
    uint carry_lo = lo < p0 ? 1u : 0u;

    hi = p3 + (mid >> 16) + carry_mid + carry_lo;
}
