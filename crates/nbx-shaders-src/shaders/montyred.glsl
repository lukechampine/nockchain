layout(local_size_x = 256) in;

#define U64 uint64_t
#define U128 uint128_t

layout(std140, binding = 0) uniform Globals {
    uint off;
    uint numElems;
};

layout(std430, binding = 1) buffer OutputBuf {
    U64 outBuf[];
};

layout(std430, binding = 2) buffer InputBuf {
    U64 inpBuf[];
};

void main() {
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    uint curIdx = off + idx;

    outBuf[curIdx] = montReduction(U128(inpBuf[curIdx], U64(0)));

    /*if (curIdx == 0) {
        debugPrintfEXT("%08x%08x -> %08x%08x", uint(inpBuf[curIdx] >> 32), uint(inpBuf[curIdx]), uint(outBuf[curIdx] >> 32), uint(outBuf[curIdx]));
    }*/
}
