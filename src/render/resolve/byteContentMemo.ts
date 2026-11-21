function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a === b) return true;
  const words = Math.floor(a.length / 4);
  const av = new DataView(a.buffer, a.byteOffset, a.byteLength);
  const bv = new DataView(b.buffer, b.byteOffset, b.byteLength);
  for (let i = 0; i < words * 4; i += 4)
    if (av.getUint32(i) !== bv.getUint32(i)) return false;
  for (let i = words * 4; i < a.length; i++)
    if (a[i] !== b[i]) return false;
  return true;
}

/** Per-load memo for separately allocated copies of the same encoded asset.
 * The sampled key only narrows candidates; every hit verifies all bytes.
 * Variants keep different color spaces / samplers isolated. */
export function createByteContentMemo<T>() {
  const buckets = new Map<string, { bytes: Uint8Array; value: Promise<T> }[]>();
  return {
    get(bytes: Uint8Array, variant: string, load: () => Promise<T>): Promise<T> {
      let sample = 2166136261;
      const step = Math.max(1, Math.floor(bytes.length / 32));
      for (let i = 0; i < bytes.length; i += step)
        sample = Math.imul(sample ^ bytes[i], 16777619);
      const key = `${variant}:${bytes.length}:${sample}`;
      const bucket = buckets.get(key) ?? [];
      for (const entry of bucket) {
        if (equalBytes(entry.bytes, bytes))
          return entry.value;
      }
      const entry = { bytes, value: Promise.resolve().then(load) };
      bucket.push(entry);
      buckets.set(key, bucket);
      void entry.value.catch(() => {
        const index = bucket.indexOf(entry);
        if (index >= 0) bucket.splice(index, 1);
      });
      return entry.value;
    },
  };
}
