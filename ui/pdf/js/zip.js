/* Minimal ZIP writer (stored, no compression): enough to hand several results over as one file. */
"use strict";

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(bytes) {
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++)
    c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/* files: [{name, bytes}] -> Uint8Array */
function makeZip(files) {
  const enc = new TextEncoder();
  const parts = [];
  const central = [];
  let offset = 0;
  const push = (u8) => {
    parts.push(u8);
    offset += u8.length;
  };
  for (const f of files) {
    const name = enc.encode(f.name);
    const crc = crc32(f.bytes);
    const head = new DataView(new ArrayBuffer(30));
    head.setUint32(0, 0x04034b50, true);
    head.setUint16(4, 20, true);
    head.setUint16(6, 0x0800, true); // UTF-8 names
    head.setUint16(10, 0x6000, true); // fixed time
    head.setUint16(12, 0x5a21, true); // fixed date
    head.setUint32(14, crc, true);
    head.setUint32(18, f.bytes.length, true);
    head.setUint32(22, f.bytes.length, true);
    head.setUint16(26, name.length, true);
    central.push({ name, crc, size: f.bytes.length, offset });
    push(new Uint8Array(head.buffer));
    push(name);
    push(f.bytes);
  }
  const start = offset;
  for (const c of central) {
    const h = new DataView(new ArrayBuffer(46));
    h.setUint32(0, 0x02014b50, true);
    h.setUint16(4, 20, true);
    h.setUint16(6, 20, true);
    h.setUint16(8, 0x0800, true);
    h.setUint16(12, 0x6000, true);
    h.setUint16(14, 0x5a21, true);
    h.setUint32(16, c.crc, true);
    h.setUint32(20, c.size, true);
    h.setUint32(24, c.size, true);
    h.setUint16(28, c.name.length, true);
    h.setUint32(42, c.offset, true);
    push(new Uint8Array(h.buffer));
    push(c.name);
  }
  const end = new DataView(new ArrayBuffer(22));
  end.setUint32(0, 0x06054b50, true);
  end.setUint16(8, central.length, true);
  end.setUint16(10, central.length, true);
  end.setUint32(12, offset - start, true);
  end.setUint32(16, start, true);
  push(new Uint8Array(end.buffer));
  return new Blob(parts, { type: "application/zip" });
}
