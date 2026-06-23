/* tslint:disable */
/* eslint-disable */

export class Compressor {
    free(): void;
    [Symbol.dispose](): void;
    compress(input: Uint8Array): Uint8Array;
    constructor();
    static withDict(dict: Uint8Array): Compressor;
}

export class Decompressor {
    free(): void;
    [Symbol.dispose](): void;
    decompress(input: Uint8Array, uncompressed_size: number): Uint8Array;
    constructor();
    static withDict(dict: Uint8Array): Decompressor;
}

export class DictTrainer {
    free(): void;
    [Symbol.dispose](): void;
    addSample(data: Uint8Array): void;
    constructor(max_dict_size: number);
    sampleCount(): number;
    train(): Uint8Array;
}

export function compress(input: Uint8Array): Uint8Array;

export function compressBound(input_len: number): number;

export function decompress(input: Uint8Array, uncompressed_size: number): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_compressor_free: (a: number, b: number) => void;
    readonly __wbg_decompressor_free: (a: number, b: number) => void;
    readonly __wbg_dicttrainer_free: (a: number, b: number) => void;
    readonly compress: (a: number, b: number, c: number) => void;
    readonly compressBound: (a: number) => number;
    readonly compressor_compress: (a: number, b: number, c: number, d: number) => void;
    readonly compressor_new: () => number;
    readonly compressor_withDict: (a: number, b: number) => number;
    readonly decompress: (a: number, b: number, c: number, d: number) => void;
    readonly decompressor_decompress: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly decompressor_new: () => number;
    readonly decompressor_withDict: (a: number, b: number) => number;
    readonly dicttrainer_addSample: (a: number, b: number, c: number, d: number) => void;
    readonly dicttrainer_new: (a: number) => number;
    readonly dicttrainer_sampleCount: (a: number, b: number) => void;
    readonly dicttrainer_train: (a: number, b: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
