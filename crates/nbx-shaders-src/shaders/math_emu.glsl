#if defined(EMULATE_EXTENDED_MATH) && (EMULATE_EXTENDED_MATH > 0)
// x, y, msb, lsb
void umulEx(U32 a, U32 b, out U32 result_hi, out U32 result_lo) {
    U32 a_low = a & 0xFFFFu;
    U32 a_high = a >> 16;
    U32 b_low = b & 0xFFFFu;
    U32 b_high = b >> 16;
    
    U32 ll = a_low * b_low;    // Low * Low
    U32 lh = a_low * b_high;   // Low * High
    U32 hl = a_high * b_low;   // High * Low
    U32 hh = a_high * b_high;  // High * High
    
    // Start with ll in low part, hh in high part
    result_lo = ll;
    result_hi = hh;
    
    // Add lh shifted by 16 bits
    result_lo += lh << 16;
    U32 carry1 = U32(lessThan(result_lo, (lh << 16)));
    result_hi += (lh >> 16) + carry1;
    
    // Add hl shifted by 16 bits
    U32 old_lo = result_lo;
    result_lo += hl << 16;
    U32 carry2 = U32(lessThan(result_lo, old_lo));
    result_hi += (hl >> 16) + carry2;
}
#else
void umulEx(U32 a, U32 b, out U32 result_hi, out U32 result_lo) {
    umulExtended(a, b, result_hi, result_lo);
}
#endif
