import {
  initSync,
  compress as wasmCompress,
  decompress as wasmDecompress,
  compressBound as wasmCompressBound,
  Compressor,
  Decompressor,
  DictTrainer,
} from "./pkg/lz4rip_wasm.js";

export { Compressor, Decompressor, DictTrainer };

let initialized = false;

/**
 * Initialize the WASM module. Must be called before any other function.
 */
export async function init(): Promise<void> {
  if (initialized) return;

  const wasmUrl = new URL("./pkg/lz4rip.wasm", import.meta.url);
  const response = await fetch(wasmUrl);
  const bytes = await response.arrayBuffer();
  initSync({ module: new WebAssembly.Module(bytes) });
  initialized = true;
}

/**
 * Initialize synchronously with a pre-loaded WASM binary.
 */
export function initSyncFromBytes(bytes: BufferSource): void {
  if (initialized) return;
  initSync({ module: new WebAssembly.Module(bytes) });
  initialized = true;
}

/** Compress data using LZ4 block format. */
export function compress(input: Uint8Array): Uint8Array {
  return wasmCompress(input);
}

/**
 * Decompress LZ4 block data. The uncompressed size must be known in advance.
 */
export function decompress(
  input: Uint8Array,
  uncompressedSize: number,
): Uint8Array {
  return wasmDecompress(input, uncompressedSize);
}

/** Upper bound on compressed size for a given input length. */
export function compressBound(inputLen: number): number {
  return wasmCompressBound(inputLen);
}
