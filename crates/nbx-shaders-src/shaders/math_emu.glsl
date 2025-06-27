void umulExtended(uint a, uint b, out uint result_lo, out uint result_hi) {
    uint a_low = a & 0xFFFFu;
    uint a_high = a >> 16;
    uint b_low = b & 0xFFFFu;
    uint b_high = b >> 16;
    
    uint ll = a_low * b_low;    // Low * Low
    uint lh = a_low * b_high;   // Low * High
    uint hl = a_high * b_low;   // High * Low
    uint hh = a_high * b_high;  // High * High
    
    // Start with ll in low part, hh in high part
    result_lo = ll;
    result_hi = hh;
    
    // Add lh shifted by 16 bits
    result_lo += lh << 16;
    uint carry1 = result_lo < (lh << 16) ? 1u : 0u;
    result_hi += (lh >> 16) + carry1;
    
    // Add hl shifted by 16 bits
    uint old_lo = result_lo;
    result_lo += hl << 16;
    uint carry2 = result_lo < old_lo ? 1u : 0u;
    result_hi += (hl >> 16) + carry2;
}
