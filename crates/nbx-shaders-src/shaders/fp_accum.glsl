layout(local_size_x = 256) in;

#define PRINT_PELT(f, v) debugPrintfEXT("%08x%08x %08x%08x %08x%08x", uint(v.x >> 32), uint(v.x), uint(v.y >> 32), uint(v.y), uint(v.z >> 32), uint(v.z))

layout(std140, binding = 0) uniform Globals {
    uint i;
    uint numElems;
    uint inpOff1;
    uint inpOff2;
};

layout(std430, binding = 1) buffer OutputBuf {
    Felt outBuf[];
};

layout(std430, binding = 2) buffer InputBuf {
    Felt inpBuf[];
};

void main() {
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    uint off1 = inpOff1 + i + idx;
    uint off2 = inpOff2 + i + idx;

    Felt res = fadd(outBuf[off1], inpBuf[off2]);

    /*if (off1 == 0) {
        debugPrintfEXT("%u %u %u %u", off1, off2, inpOff1, inpOff2);
        PRINT_PELT("", outBuf[off1]);
        PRINT_PELT("", inpBuf[off2]);
        PRINT_PELT("", res);
    }*/

    outBuf[off1] = res;
}
