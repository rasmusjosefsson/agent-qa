'use strict';
// Serve the bundled Chrome extension as a single-file download.
//
// Store-method (uncompressed) zip written by hand so the endpoint needs no
// dependency and no temp file: the extension is ~140KB of text, compression
// buys almost nothing. Deterministic output (sorted names, fixed mtime) so
// repeated downloads hash identically.

const fsp = require('node:fs/promises');
const path = require('node:path');

// CRC-32 (IEEE) — table built once. zlib.crc32 exists on newer Node but the
// launcher still supports versions without it.
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

// Resolve the extension dir: packaged copy inside the npm tarball first,
// then the repo checkout root (dev sessions / repo installs).
async function findExtensionDir(startDir) {
  const candidates = [
    path.join(startDir, '..', 'extension'),
    path.join(startDir, '..', '..', '..', 'extension'),
  ];
  for (const dir of candidates) {
    try {
      const st = await fsp.stat(path.join(dir, 'manifest.json'));
      if (st.isFile()) return dir;
    } catch {
      /* try next */
    }
  }
  return null;
}

async function* walk(dir, prefix = '') {
  const entries = await fsp.readdir(dir, { withFileTypes: true });
  entries.sort((a, b) => (a.name < b.name ? -1 : 1));
  for (const ent of entries) {
    const rel = prefix ? `${prefix}/${ent.name}` : ent.name;
    if (ent.isDirectory()) yield* walk(path.join(dir, ent.name), rel);
    else if (ent.isFile()) yield rel;
  }
}

// Build a complete zip in memory. Returns null when the dir is missing.
async function zipExtensionDir(dir) {
  const files = [];
  for await (const rel of walk(dir)) {
    const abs = path.join(dir, rel);
    files.push({ rel, data: await fsp.readFile(abs) });
  }
  if (!files.length) return null;

  const parts = [];
  const central = [];
  let offset = 0;
  // Fixed DOS date (2024-01-01 00:00) for deterministic output.
  const dosTime = 0;
  const dosDate = ((2024 - 1980) << 9) | (1 << 5) | 1;

  for (const f of files) {
    const name = Buffer.from(`extension/${f.rel}`, 'utf8');
    const crc = crc32(f.data);
    const head = Buffer.alloc(30);
    head.writeUInt32LE(0x04034b50, 0);
    head.writeUInt16LE(20, 4); // version needed
    head.writeUInt16LE(0x0800, 6); // UTF-8 flag
    head.writeUInt16LE(0, 8); // store
    head.writeUInt16LE(dosTime, 10);
    head.writeUInt16LE(dosDate, 12);
    head.writeUInt32LE(crc, 14);
    head.writeUInt32LE(f.data.length, 18);
    head.writeUInt32LE(f.data.length, 22);
    head.writeUInt16LE(name.length, 26);
    head.writeUInt16LE(0, 28);
    parts.push(head, name, f.data);
    central.push({ name, crc, size: f.data.length, offset });
    offset += head.length + name.length + f.data.length;
  }

  const cdStart = offset;
  for (const c of central) {
    const e = Buffer.alloc(46);
    e.writeUInt32LE(0x02014b50, 0);
    e.writeUInt16LE(20, 4); // version made by
    e.writeUInt16LE(20, 6); // version needed
    e.writeUInt16LE(0x0800, 8);
    e.writeUInt16LE(0, 10);
    e.writeUInt16LE(dosTime, 12);
    e.writeUInt16LE(dosDate, 14);
    e.writeUInt32LE(c.crc, 16);
    e.writeUInt32LE(c.size, 20);
    e.writeUInt32LE(c.size, 24);
    e.writeUInt16LE(c.name.length, 28);
    e.writeUInt32LE(c.offset, 42);
    parts.push(e, c.name);
    offset += e.length + c.name.length;
  }

  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(central.length, 8);
  end.writeUInt16LE(central.length, 10);
  end.writeUInt32LE(offset - cdStart, 12);
  end.writeUInt32LE(cdStart, 16);
  parts.push(end);
  return Buffer.concat(parts);
}

module.exports = { findExtensionDir, zipExtensionDir };
