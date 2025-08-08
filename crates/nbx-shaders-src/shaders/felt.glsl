#if !defined(FELT_GLSL)
#define FELT_GLSL

struct Felt {
    uint64_t x;
    uint64_t y;
    uint64_t z;
};

Felt fadd(Felt a, Felt b) {
    return Felt(
        badd(a.x, b.x),
        badd(a.y, b.y),
        badd(a.z, b.z)
    );
}

Felt fsub(Felt a, Felt b) {
    return Felt(
        bsub(a.x, b.x),
        bsub(a.y, b.y),
        bsub(a.z, b.z)
    );
}

Felt fmul(Felt a, Felt b) {
    uint64_t a0b0 = bmul(a.x, b.x);
    uint64_t a1b1 = bmul(a.y, b.y);
    uint64_t a2b2 = bmul(a.z, b.z);
    uint64_t a0b1_a1b0 = bsub(bsub(bmul(badd(a.x, a.y), badd(b.x, b.y)), a0b0), a1b1);
    uint64_t a1b2_a2b1 = bsub(bsub(bmul(badd(a.y, a.z), badd(b.y, b.z)), a1b1), a2b2);
    uint64_t a0b2_a2b0 = bsub(bsub(bmul(badd(a.x, a.z), badd(b.x, b.z)), a0b0), a2b2);

    return Felt(
        bsub(a0b0, a1b2_a2b1),
        bsub(badd(a0b1_a1b0, a1b2_a2b1), a2b2),
        badd(badd(a0b2_a2b0, a1b1), a2b2)
    );
}

#endif
