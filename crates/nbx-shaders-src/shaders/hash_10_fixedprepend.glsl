layout(local_size_x = 256) in;
layout(std140, binding = 0) uniform Globals {
    uint startIdx;
    uint numHashes;
    uint64_t p1;
    u64vec2 p2;
    u64vec2 p3;
};
// Output of the shader.
layout(std430, binding = 1) buffer OutputBuf {
    uint64_t outBuf[];
};
// Input to the shader. The length of the array is determined by what buffer is bound.
//
// Out of bounds accesses
layout(std430, binding = 2) readonly buffer InputBuf {
    uint64_t inpBuf[];
};

// Ideal workgroup size depends on the hardware, the workload, and other factors. However, it should
// _generally_ be a multiple of 64. Common sizes are 64x1x1, 256x1x1; or 8x8x1, 16x16x1 for 2D workloads.
void main() {
    // While compute invocations are 3d, we're only using one dimension.
    uint idx = gl_GlobalInvocationID.x;

    // Because we're using a workgroup size of 64, if the input size isn't a multiple of 64,
    // we will have some "extra" invocations. This is fine, but we should tell them to stop
    // to avoid out-of-bounds accesses.
    if (idx >= numHashes) {
        return;
    }

    uint off = (startIdx + idx) * 5;

    Sponge tmp = fixedSponge;

    spongeSet(tmp, 0, p1);
    spongeSet(tmp, 1, p2[0]);
    spongeSet(tmp, 2, p2[1]);
    spongeSet(tmp, 3, p3[0]);
    spongeSet(tmp, 4, p3[1]);

    for (uint i = 0; i < 5; i += 1) {
        spongeSet(tmp, i + 5, inpBuf[off + i]);
    }

    /*if (off == 0) {
        tip5SpongePrint(tmp);
    }*/

    tip5Permute(tmp);

    /*if (off == 0) {
        tip5SpongePrint(tmp);
    }*/

    for (uint i = 0; i < 5; i += 1) {
        outBuf[off + i] = spongeGet(tmp, i);
    }
}
