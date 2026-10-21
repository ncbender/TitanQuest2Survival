"""Tiny minidump reader over MemoryList (5) and Memory64List (9) streams."""
import struct, bisect

class Dump:
    def __init__(self, path):
        self.data = open(path, 'rb').read()
        n, rva = struct.unpack_from('<II', self.data, 8)
        self.ranges = []
        for i in range(n):
            typ, size, loc = struct.unpack_from('<III', self.data, rva + 12 * i)
            if typ == 5:
                cnt = struct.unpack_from('<I', self.data, loc)[0]
                for j in range(cnt):
                    start, sz, off = struct.unpack_from('<QII', self.data, loc + 4 + 16 * j)
                    self.ranges.append((start, sz, off))
            elif typ == 9:
                cnt, off = struct.unpack_from('<QQ', self.data, loc)
                for j in range(cnt):
                    start, sz = struct.unpack_from('<QQ', self.data, loc + 16 + 16 * j)
                    self.ranges.append((start, sz, off)); off += sz
        self.ranges.sort()
        self.starts = [r[0] for r in self.ranges]

    def read(self, addr, n):
        i = bisect.bisect_right(self.starts, addr) - 1
        if i < 0: return None
        start, sz, off = self.ranges[i]
        if addr + n > start + sz: return None
        return self.data[off + addr - start: off + addr - start + n]

    def q(self, addr):
        b = self.read(addr, 8)
        return struct.unpack('<Q', b)[0] if b else None
