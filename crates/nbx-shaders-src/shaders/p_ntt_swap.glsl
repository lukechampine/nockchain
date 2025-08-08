layout(local_size_x = 256) in;

#define U64 uint64_t

layout(std140, binding = 0) uniform Globals {
    uint off;
    uint i;
    uint polyLen;
    uint numElems;
};

layout(std430, binding = 1) buffer OutputBuf {
    U64 buf[];
};

layout(std430, binding = 2) buffer SwapMaskBuf {
    U64 swapMask[];
};

layout(std430, binding = 3) buffer SwapIdxBuf {
    uint swapIdx[];
};

void main() {
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    uint elemIdx = (i + idx) % polyLen;
    uint polyIdx = (i + idx) / polyLen;

    U64 mask = swapMask[elemIdx];

    uint coff = off + polyIdx * polyLen + elemIdx;
    uint roff = off + polyIdx * polyLen + swapIdx[elemIdx];

    U64 a = buf[coff];
    U64 b = buf[roff];

    /*if (coff >= 3069 && coff <= 3071) {
        debugPrintfEXT("SWAP %08x%08x %u %u %u %u | %08x%08x <-> %08x%08x", uint(mask >> 32), uint(mask), elemIdx, polyIdx, coff, roff, uint(a >> 32), uint(a), uint(b >> 32), uint(b));
    }*/

    // TODO: make this branchless. We can't do xor trick, because offsets might overlap with other threads.
    if (mask != U64(0)) {
        buf[coff] = b;
        buf[roff] = a;
    }

}
