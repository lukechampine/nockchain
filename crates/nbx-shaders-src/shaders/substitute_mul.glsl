#define U64 u64vec2
#define U64SZ 2

struct SubstituteOps {
    uint64_t scal;
    uint vars;
    uint numVarsAndIterId;
};

struct SubstituteOp {
    uint chunk;
    uint e;
};

layout(local_size_x = 256) in;
layout(std430, binding = 0) readonly buffer OpsBuf {
    SubstituteOps allOps[];
};
layout(std140, binding = 1) uniform Globals {
    uint polyLen;
    uint chunksPerBuf;
    uint opsOffset;
    uint numOps;
    uint inpOffset;
    uint accumMask;
    uint outOffset;
};
// Output of the shader.
layout(std430, binding = 2) buffer OutputBuf {
    U64 outBuf[];
};
// Actual substitutions to be performed. Fixed
layout(std430, binding = 3) readonly buffer SubsBuf {
    SubstituteOp subs[];
};
// Input to the shader. The length of the array is determined by what buffer is bound.
//
// Out of bounds accesses
layout(std430, binding = 4) buffer InputBuf {
    U64 inpBuf[];
};

// Ideal workgroup size depends on the hardware, the workload, and other factors. However, it should
// _generally_ be a multiple of 64. Common sizes are 64x1x1, 256x1x1; or 8x8x1, 16x16x1 for 2D workloads.
void main() {
    // While compute invocations are 3d, we're only using one dimension.
    uint idx = opsOffset + gl_WorkGroupID.x * U64SZ * gl_WorkGroupSize.x / polyLen;

    // Because we're using a workgroup size of 64, if the input size isn't a multiple of 64,
    // we will have some "extra" invocations. This is fine, but we should tell them to stop
    // to avoid out-of-bounds accesses.
    if (idx >= opsOffset + numOps) {
        //debugPrintfEXT("RETURN");
        return;
    }

    uint bufOffset = gl_GlobalInvocationID.x % (polyLen / U64SZ);

    SubstituteOps ops = allOps[idx];

    uint numVars = ops.numVarsAndIterId & 0xffffu;
    uint iterId = (ops.numVarsAndIterId >> 16) % chunksPerBuf;
    uint pt = 4000;

    /*if (bufOffset == 0 && idx > pt) {
        debugPrintfEXT("1. %u %x%x %u %x %x", uint(idx), uint(ops.scal >> 32), uint(ops.scal), ops.vars, numVars, iterId);
        /*for (uint i = 0; i < 4; i += 1) {
            debugPrintfEXT("-- %u: %x%x %u %x", i, uint(allOps[i].scal >> 32), uint(allOps[i].scal), allOps[i].vars, allOps[i].numVarsAndIterId);
        }* /
    }*/

    U64 ret = U64(ops.scal);

    for (uint i = 0; i < numVars; i += 1) {
        SubstituteOp op = subs[ops.vars + i];
        uint chunk = op.chunk % chunksPerBuf;
        /*if (bufOffset == 0 && idx > pt) {
            debugPrintfEXT("%u: %u (%u) %u", i, op.chunk, chunk, op.e);
        }*/
        U64 v = inpBuf[inpOffset + chunk * (polyLen / U64SZ) + bufOffset];
        for (uint j = 0; j < op.e; j += 1) {
            ret = montiply(ret, v);
        }
    }

    U64 mask = U64(accumMask) | (U64(accumMask) << 32);
    uint obOffset = outOffset + iterId * (polyLen / U64SZ) + bufOffset;
    U64 accum = (outBuf[obOffset] & mask) | (oneMelt & ~mask);
    U64 accumed = montiply(accum, ret);
    /*if (obOffset == 0 || obOffset == 4194304) {
        debugPrintfEXT("2. %u %x%x %x%x | %x%x | %x%x", obOffset, uint(mask >> 32), uint(mask), uint(accum >> 32), uint(accum), uint(ret >> 32), uint(ret), uint(accumed >> 32), uint(accumed));
    }*/
    outBuf[obOffset] = accumed;
}
