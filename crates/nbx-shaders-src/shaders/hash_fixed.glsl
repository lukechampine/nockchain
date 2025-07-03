layout(local_size_x = 256) in;
// Input to the shader. The length of the array is determined by what buffer is bound.
//
// Out of bounds accesses
layout(std430, binding = 0) readonly buffer InputBuf {
    uint64_t inpBuf[];
};
layout(std430, binding = 1) readonly buffer OpsBuf {
    ReduceOp ops[];
};
layout(std140, binding = 2) uniform Globals {
    uint globalOffset;
};
// Output of the shader.
layout(std430, binding = 3) buffer OutputBuf {
    uint64_t outBuf[];
};

// Ideal workgroup size depends on the hardware, the workload, and other factors. However, it should
// _generally_ be a multiple of 64. Common sizes are 64x1x1, 256x1x1; or 8x8x1, 16x16x1 for 2D workloads.
void main() {
    // While compute invocations are 3d, we're only using one dimension.
    uint idx = gl_GlobalInvocationID.x + globalOffset;

    // Because we're using a workgroup size of 64, if the input size isn't a multiple of 64,
    // we will have some "extra" invocations. This is fine, but we should tell them to stop
    // to avoid out-of-bounds accesses.
    uint array_length = ops.length();

    if (idx >= array_length) {
        return;
    }

    // Do the multiply by two and write to the output.
    ReduceOp op = ops[idx];

    Sponge tmp = fixedSponge;

    for (uint i = 0; i < tip5Rate; i += 1) {
        tmp.s[i / 4][i % 4] = inpBuf[op.source + i];
    }

    tip5Permute(tmp);

    for (uint i = 0; i < 5; i += 1) {
        outBuf[op.destination + i] = tmp.s[i / 4][i % 4];
    }
}
