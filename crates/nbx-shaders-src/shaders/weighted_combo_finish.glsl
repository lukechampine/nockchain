layout(local_size_x = 256) in;

#define PRINT_PELT(f, v) debugPrintfEXT("%08x%08x %08x%08x %08x%08x", uint(v.x >> 32), uint(v.x), uint(v.y >> 32), uint(v.y), uint(v.z >> 32), uint(v.z))

layout(std140, binding = 0) uniform Globals {
    uint64_t invLenX;
    uint64_t invLenY;
    uint64_t invLenZ;
    uint i;
    uint numElems;
    uint inPolyLen;
    uint outPolyLen;
    uint weightsOff;
    uint _pad;
};

layout(std430, binding = 1) buffer OutputBuf {
    Felt outBuf[];
};

layout(std430, binding = 2) buffer InpBuf {
    Felt inpBuf[];
};

layout(std430, binding = 3) buffer LeadBuf {
    Felt leadBuf[];
};

layout(std430, binding = 4) buffer WeightsBuf {
    Felt weightsBuf[];
};

void main() {
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    uint polyIdx = (i + idx) / outPolyLen;

    Felt invLen = Felt(invLenX, invLenY, invLenZ);
    Felt lead = leadBuf[polyIdx];
    Felt weight = weightsBuf[weightsOff + polyIdx];
    Felt scal = fmul(fmul(invLen, lead), weight);

    uint elemIdx = (i + idx) % outPolyLen;
    uint inElemIdx = polyIdx * inPolyLen + outPolyLen - 1 - elemIdx;
    uint outOff = polyIdx * outPolyLen + elemIdx;

    Felt res = fmul(scal, inpBuf[inElemIdx]);

    /*if (outOff == 0) {
        debugPrintfEXT("%u %u %u %u %08x%08x %08x%08x", polyIdx, elemIdx, inElemIdx, outOff, uint(scal.x >> 32), uint(scal.x), uint(inpBuf[inElemIdx].x >> 32), uint(inpBuf[inElemIdx].x));
        PRINT_PELT("", invLen);
        PRINT_PELT("", lead);
        PRINT_PELT("", weight);
        PRINT_PELT("", scal);
        PRINT_PELT("", res);
    }*/

    outBuf[outOff] = res;
}
