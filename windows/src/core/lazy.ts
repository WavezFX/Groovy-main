// Round 3D item 9: code that is its own chunk (a Settings page, a grown
// island tile), loaded the first time it is needed and kept after.

/** The module, imported on the first call only; later calls get the same promise. */
export function once<T>(load: () => Promise<T>): () => Promise<T> {
  let module: Promise<T> | null = null;
  return () => (module ??= load());
}
