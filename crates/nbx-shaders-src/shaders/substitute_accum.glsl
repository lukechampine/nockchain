layout(local_size_x = 256) in;

#define U64 u64vec2

layout(std140, binding = 0) uniform Globals {
    uint inpOffsetA;
    uint inpOffsetB;
    uint numElems;
    uint outOffset;
    uint accumMask;
};

// Output of the shader.
layout(std430, binding = 1) buffer OutputBuf {
    U64 outBuf[];
};

// Input to the shader. The length of the array is determined by what buffer is bound.
//
// Out of bounds accesses
layout(std430, binding = 2) readonly buffer InputBuf {
    U64 inpBuf[];
};

// Ideal workgroup size depends on the hardware, the workload, and other factors. However, it should
// _generally_ be a multiple of 64. Common sizes are 64x1x1, 256x1x1; or 8x8x1, 16x16x1 for 2D workloads.
void main() {
    // While compute invocations are 3d, we're only using one dimension.
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    U64 a = inpBuf[inpOffsetA + idx];
    U64 b = inpBuf[inpOffsetB + idx];

    U64 mask = U64(accumMask) | (U64(accumMask) << 32);
    U64 prev = (outBuf[outOffset + idx] & mask) | (zeroMelt & ~mask);

    U64 r2 = badd(a, b);
    U64 res = badd(r2, prev);

    /*if ((idx % 0x1000) == 0) {
        debugPrintfEXT("%u %u %u %016x%016x %016x%016x %016x%016x %016x%016x %016x%016x %x", idx, inpOffsetA, inpOffsetB, uint(a >> 32), uint(a), uint(b >> 32), uint(b), uint(res >> 32), uint(res), uint(r2 >> 32), uint(r2), uint(prev >> 32), uint(prev), accumMask);
    }*/

    outBuf[outOffset + idx] = res;
}
