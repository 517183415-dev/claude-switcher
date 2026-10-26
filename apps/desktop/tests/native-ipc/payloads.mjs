// Valid 1x1 PNG. Add a legal ancillary chunk before IEND to reach each exact input length.
export function image(length) {
  const png = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII='), c => c.charCodeAt(0));
  const bytes = new Uint8Array(length);
  const iend = png.length - 12;
  bytes.set(png.subarray(0, iend));
  const size = length - png.length - 12;
  const view = new DataView(bytes.buffer);
  view.setUint32(iend, size);
  bytes.set([0x70, 0x61, 0x44, 0x64], iend + 4);
  let crc = 0xffffffff;
  for (let i = iend + 4; i < iend + 8 + size; i++) {
    crc ^= bytes[i];
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  view.setUint32(iend + 8 + size, (crc ^ 0xffffffff) >>> 0);
  bytes.set(png.subarray(iend), length - 12);
  return Array.from(bytes);
}
