layout(local_size_x = 256) in;

layout(std140, binding = 0) uniform Globals {
    uint inpOffsetA;
    uint inpOffsetB;
    uint numElems;
    uint outOffset;
    uint accumMask;
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

    if (idx >= numElems) {
        return;
    }

    uint64_t a = inpBuf[inpOffsetA + idx];
    uint64_t b = inpBuf[inpOffsetB + idx];

    uint64_t mask = uint64_t(accumMask) | (uint64_t(accumMask) << 32);
    uint64_t prev = (outBuf[outOffset + idx] & mask) | (zeroMelt & ~mask);

    uint64_t r2 = badd(a, b);
    uint64_t res = badd(r2, prev);

    /*if ((idx % 0x1000) == 0) {
        debugPrintfEXT("%u %u %u %016x%016x %016x%016x %016x%016x %016x%016x %016x%016x %x", idx, inpOffsetA, inpOffsetB, uint(a >> 32), uint(a), uint(b >> 32), uint(b), uint(res >> 32), uint(res), uint(r2 >> 32), uint(r2), uint(prev >> 32), uint(prev), accumMask);
    }*/

    outBuf[outOffset + idx] = res;
}
