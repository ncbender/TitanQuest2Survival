"""Print exception, faulting module+offset and registers of a minidump."""
import struct, ntpath, sys
data = open(sys.argv[1], 'rb').read()
n, rva = struct.unpack_from('<II', data, 8)
streams = {}
for i in range(n):
    t, s, l = struct.unpack_from('<III', data, rva + 12 * i); streams[t] = (s, l)
s, l = streams[6]
tid = struct.unpack_from('<I', data, l)[0]
code, flags, rec, addr, nparams, _ = struct.unpack_from('<IIQQII', data, l + 8)
params = struct.unpack_from('<15Q', data, l + 8 + 32)
ctx_size, ctx_rva = struct.unpack_from('<II', data, l + 8 + 152)
mods = []
s, ml = streams[4]; cnt = struct.unpack_from('<I', data, ml)[0]
for i in range(cnt):
    o = ml + 4 + 108 * i; base, size = struct.unpack_from('<QI', data, o); nrva = struct.unpack_from('<I', data, o + 20)[0]
    ln = struct.unpack_from('<I', data, nrva)[0]; mods.append((base, size, ntpath.basename(data[nrva + 4:nrva + 4 + ln].decode('utf-16le'))))
def where(a):
    for b, sz, nm in mods:
        if b <= a < b + sz: return f"{nm}+{a - b:#x}"
    return ""
print(f"exception {code:#x} at {addr:#x} {where(addr)} access {[hex(p) for p in params[:nparams]]}")
# CONTEXT (AMD64): Rax at 0x78 ... Rip at 0xF8
names = "rax rcx rdx rbx rsp rbp rsi rdi r8 r9 r10 r11 r12 r13 r14 r15 rip".split()
regs = struct.unpack_from('<17Q', data, ctx_rva + 0x78)
print("  ".join(f"{nm}={v:#x}" for nm, v in zip(names, regs)))
# return addresses on stack that land in modules
sys.path.insert(0, ntpath.dirname(__file__) or '.')
from minidump import Dump
d = Dump(sys.argv[1]); rsp = regs[4]; frames = []
for k in range(0, 0x800, 8):
    v = d.q(rsp + k)
    if v and where(v) and not where(v).startswith("ntdll"): frames.append(where(v))
print("stack code ptrs:", frames[:25])
