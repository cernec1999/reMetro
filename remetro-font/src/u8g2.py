import struct
from math import ceil, log2

ASCII_FIRST, ASCII_LAST = 32, 126


# ---------- helpers ----------
class LsbBitWriter:
    """Append bits LSB-first to a byte stream (what u8g2-fonts expects)."""

    def __init__(self):
        self.buf = bytearray()
        self.acc = 0
        self.n = 0  # how many bits currently in acc (0..7)

    def write_bits(self, value: int, bitcount: int):
        # Push 'bitcount' bits from LSB to MSB of 'value'
        for i in range(bitcount):
            bit = (value >> i) & 1  # take LSB first
            self.acc |= (bit & 1) << self.n
            self.n += 1
            if self.n == 8:
                self.buf.append(self.acc)
                self.acc = 0
                self.n = 0

    def finish(self) -> bytes:
        if self.n:
            self.buf.append(self.acc)
            self.acc = 0
            self.n = 0
        return bytes(self.buf)


def ceil_log2p1(v: int) -> int:
    # bit width for unsigned values in [0..v], v>=0
    return 0 if v <= 0 else max(1, int(ceil(log2(v + 1))))


def encode_signed(val: int, bits: int) -> int:
    """u8g2 signed bias: unsigned = val + 2^(bits-1)"""
    bias = 1 << (bits - 1)
    return (val + bias) & ((1 << bits) - 1)


def flatten_bits_tight(rows):
    """Return (bits, w, h) where bits is exactly w*h (row-major), no padding."""
    if not rows:
        return [], 0, 0
    h = len(rows)
    w = len(rows[0])
    for r in rows:
        if len(r) != w:
            raise ValueError("Non-rectangular glyph")
    out = []
    for y in range(h):
        out.extend(1 if px else 0 for px in rows[y])
    return out, w, h


def rle_encode_to_writer(bit_list, m0_bits, m1_bits, writer):
    """RLE encode directly to an existing LsbBitWriter (no padding)."""
    if not bit_list:
        return

    # Build alternating runs starting with zeros
    runs = []
    i = 0
    z = 0
    while i < len(bit_list) and bit_list[i] == 0:
        z += 1
        i += 1
    runs.append(z)
    cur = 1
    cnt = 0
    while i < len(bit_list):
        b = bit_list[i]
        if b == cur:
            cnt += 1
        else:
            runs.append(cnt)
            cur ^= 1
            cnt = 1
        i += 1
    runs.append(cnt)
    if len(runs) & 1:
        runs.append(0)

    max0 = (1 << m0_bits) - 1
    max1 = (1 << m1_bits) - 1
    for j in range(0, len(runs), 2):
        zlen = runs[j]
        olen = runs[j + 1]

        # Emit continuation chunks for long zero runs first (do not consume ones yet)
        while zlen > max0:
            writer.write_bits(max0, m0_bits)
            writer.write_bits(0, m1_bits)
            writer.write_bits(0, 1)  # continuation (reference style)
            zlen -= max0

        # Emit continuation chunks for long one runs
        while olen > max1:
            writer.write_bits(zlen, m0_bits)
            writer.write_bits(max1, m1_bits)
            writer.write_bits(0, 1)  # continuation (reference style)
            olen -= max1
            zlen = 0

        # Final chunk for this pair
        writer.write_bits(zlen, m0_bits)
        writer.write_bits(olen, m1_bits)
        writer.write_bits(0, 1)  # stop


def rle_encode(bit_list, m0_bits, m1_bits):
    """RLE encode a bit list to bytes (for backward compatibility)."""
    writer = LsbBitWriter()
    rle_encode_to_writer(bit_list, m0_bits, m1_bits, writer)
    return writer.finish()


def export_u8g2_from_font_data(font_data: dict, out_path: str):
    """
    Build a standard U8g2 binary from { 'A': [[0/1,...], ...], ... }.
    Rules:
      - Varbits order: W, H, X, Y, D
      - All bit fields and RLE are **LSB-first** within each byte
      - Signed fields are biased (encode_signed)
      - Bitmap is a tight W*H bit list (no row padding)
      - Offsets for 'A', 'a', and 0x0100 are big-endian, relative to end-of-header
    """

    # ---------- collect glyphs ----------
    glyphs = []
    max_w = 0
    max_h = 0
    for cp in range(ASCII_FIRST, ASCII_LAST + 1):
        ch = chr(cp)
        rows = font_data.get(ch)
        if not rows:
            continue
        bits, w, h = flatten_bits_tight(rows)  # NO padding
        glyphs.append(
            {
                "cp": cp,
                "w": w,
                "h": h,
                "xoff": 0,
                "yoff": 0,
                "delta": w + 1,  # 1px tracking
                "bits": bits,
            }
        )
        max_w = max(max_w, w)
        max_h = max(max_h, h)

    if not glyphs:
        raise ValueError("No ASCII glyphs present")

    # ---------- header widths ----------
    bitcntW = ceil_log2p1(max_w)
    bitcntH = ceil_log2p1(max_h)
    bitcntX = 1  # keep >=1 so signed decode is defined
    bitcntY = ceil_log2p1(max_h)  # Use height-based for Y like reference
    max_delta = max(g["delta"] for g in glyphs)
    bitcntD = ceil_log2p1(max_delta) + 1  # signed range must cover +max_delta

    # RLE field widths (use smaller values like reference)
    m0, m1 = 2, 2

    # ---------- encode glyph payloads ----------
    payloads = []  # (cp, bytes)
    starts = []  # start offsets after 23-byte header
    raw = bytearray()

    for g in glyphs:
        # Combined varbits + RLE in single bit stream (no padding between them)
        bw = LsbBitWriter()
        # Order: W, H, X, Y, D - X/Y/Delta are SIGNED (biased)
        bw.write_bits(g["w"], bitcntW)
        bw.write_bits(g["h"], bitcntH)
        bw.write_bits(encode_signed(g["xoff"], bitcntX), bitcntX)
        bw.write_bits(encode_signed(g["yoff"], bitcntY), bitcntY)
        bw.write_bits(encode_signed(g["delta"], bitcntD), bitcntD)

        # RLE immediately follows varbits (no padding)
        rle_encode_to_writer(g["bits"], m0, m1, bw)

        glyph_data = bw.finish()
        payloads.append((g["cp"], glyph_data))

    # prepend per-glyph (unicode byte, 1-byte jump), then backpatch jumps
    for cp, pay in payloads:
        starts.append(len(raw))
        raw.append(cp & 0xFF)
        raw.append(0)  # jump placeholder
        raw.extend(pay)
    for i, (cp, _pay) in enumerate(payloads):
        start = starts[i]
        end = starts[i + 1] if i + 1 < len(starts) else len(raw)
        jump = end - start
        if not 1 <= jump <= 255:
            raise ValueError(f"Glyph U+{cp:04X} too large for 1-byte jump: {jump}")
        raw[start + 1] = jump

    # ---------- 23-byte header ----------
    header = bytearray(23)
    header[0] = len(payloads) & 0xFF  # glyph count
    header[1] = 0  # bbox mode: use 0 like reference
    header[2] = m0 & 0xFF
    header[3] = m1 & 0xFF
    header[4] = bitcntW & 0xFF
    header[5] = bitcntH & 0xFF
    header[6] = bitcntX & 0xFF
    header[7] = bitcntY & 0xFF
    header[8] = bitcntD & 0xFF
    # bbox/meta (not critical for rendering)
    header[9] = max_w & 0xFF
    header[10] = max_h & 0xFF
    header[11] = 0
    header[12] = 0  # bbox_y: use 0 like reference
    header[13] = max_h & 0xFF  # ascent (A)
    header[14] = 0  # descent (g)
    header[15] = max_h & 0xFF  # ascent '('
    header[16] = 0  # descent ')'

    # Big-endian offsets from end-of-header to 'A', 'a', 0x0100
    def find_off(cp: int) -> int:
        try:
            idx = [c for (c, _) in payloads].index(cp)
            return starts[idx]
        except ValueError:
            return len(raw)

    off_A = find_off(0x41)
    off_a = find_off(0x61)
    off_100 = len(raw)
    struct.pack_into(">H", header, 17, off_A)
    struct.pack_into(">H", header, 19, off_a)
    struct.pack_into(">H", header, 21, off_100)

    # ---------- write ----------
    with open(out_path, "wb") as f:
        f.write(header)
        f.write(raw)
