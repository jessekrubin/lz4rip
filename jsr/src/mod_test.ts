import {
  assert,
  assertEquals,
  assertThrows,
} from "https://deno.land/std@0.224.0/assert/mod.ts";
import {
  compress,
  compressBound,
  Compressor,
  decompress,
  Decompressor,
  DictTrainer,
  init,
} from "./mod.ts";

Deno.test("init", async () => {
  await init();
});

// --- One-shot ---

Deno.test("one-shot round-trip", () => {
  const data = new TextEncoder().encode("hello world, hello lz4!".repeat(100));
  const compressed = compress(data);
  assert(compressed.length < data.length);
  const decompressed = decompress(compressed, data.length);
  assertEquals(decompressed, data);
});

Deno.test("compressBound", () => {
  const data = new TextEncoder().encode("compress bound payload".repeat(50));
  const bound = compressBound(data.length);
  assert(bound >= data.length);
  assert(compress(data).length <= bound);
});

Deno.test("empty input", () => {
  const empty = new Uint8Array(0);
  const compressed = compress(empty);
  const decompressed = decompress(compressed, 0);
  assertEquals(decompressed, empty);
});

Deno.test("incompressible data", () => {
  const random = crypto.getRandomValues(new Uint8Array(4096));
  const compressed = compress(random);
  const decompressed = decompress(compressed, random.length);
  assertEquals(decompressed, random);
});

// --- Stateful ---

Deno.test("stateful compressor", () => {
  const compressor = new Compressor();
  const data1 = new TextEncoder().encode("first message".repeat(50));
  const data2 = new TextEncoder().encode("second message".repeat(50));

  const c1 = compressor.compress(data1);
  const c2 = compressor.compress(data2);

  assertEquals(decompress(c1, data1.length), data1);
  assertEquals(decompress(c2, data2.length), data2);

  compressor.free();
});

Deno.test("stateful decompressor", () => {
  const data1 = new TextEncoder().encode("decompress test 1".repeat(50));
  const data2 = new TextEncoder().encode("decompress test 2".repeat(50));

  const c1 = compress(data1);
  const c2 = compress(data2);

  const decompressor = new Decompressor();
  assertEquals(decompressor.decompress(c1, data1.length), data1);
  assertEquals(decompressor.decompress(c2, data2.length), data2);

  decompressor.free();
});

// --- Dictionary ---

Deno.test("dict round-trip", () => {
  const dict = new TextEncoder().encode(
    '{"ts":"2026-04-27","level":"INFO","service":"api"}'.repeat(20),
  );

  const compressor = Compressor.withDict(dict);
  const decompressor = Decompressor.withDict(dict);

  const data = new TextEncoder().encode(
    '{"ts":"2026-04-27","level":"INFO","service":"api","msg":"ok"}'.repeat(10),
  );
  const compressed = compressor.compress(data);
  const decompressed = decompressor.decompress(compressed, data.length);
  assertEquals(decompressed, data);

  const compressedPlain = compress(data);
  assert(
    compressed.length < compressedPlain.length,
    `dict ${compressed.length} should beat plain ${compressedPlain.length}`,
  );

  compressor.free();
  decompressor.free();
});

Deno.test("dict stateful contexts reuse dictionary", () => {
  const dict = new TextEncoder().encode(
    '{"ts":"2026-04-27","level":"INFO","service":"api"}'.repeat(20),
  );
  const compressor = Compressor.withDict(dict);
  const decompressor = Decompressor.withDict(dict);
  const data1 = new TextEncoder().encode(
    '{"ts":"2026-04-27","level":"INFO","service":"api","msg":"ok1"}'.repeat(
      10,
    ),
  );
  const data2 = new TextEncoder().encode(
    '{"ts":"2026-04-27","level":"INFO","service":"api","msg":"ok2"}'.repeat(
      10,
    ),
  );

  const c1 = compressor.compress(data1);
  const c2 = compressor.compress(data2);

  assertEquals(decompressor.decompress(c1, data1.length), data1);
  assertEquals(decompressor.decompress(c2, data2.length), data2);

  compressor.free();
  decompressor.free();
});

Deno.test("dict trainer", () => {
  const trainer = new DictTrainer(2048);
  for (let i = 0; i < 200; i++) {
    trainer.addSample(
      new TextEncoder().encode(
        `{"ts":"2026-04-27T12:00:00.${i}Z","level":"INFO","service":"api-gw","status":200}`,
      ),
    );
  }
  assertEquals(trainer.sampleCount(), 200);
  const dict = trainer.train();
  assert(dict.length > 0);
});

Deno.test("dict trainer consumes on train", () => {
  const trainer = new DictTrainer(1024);
  for (let i = 0; i < 50; i++) {
    trainer.addSample(new TextEncoder().encode(`sample ${i} data`.repeat(5)));
  }
  trainer.train();
  assertThrows(
    () => trainer.addSample(new TextEncoder().encode("late sample")),
    Error,
  );
  assertThrows(() => trainer.sampleCount(), Error);
  assertThrows(() => trainer.train(), Error);
});

// --- Error paths ---

Deno.test("decompress with too-small size throws", () => {
  const data = new TextEncoder().encode("hello world".repeat(100));
  const compressed = compress(data);
  assertThrows(
    () => decompress(compressed, data.length - 100),
    Error,
  );
});

Deno.test("decompress with too-large size throws", () => {
  const data = new TextEncoder().encode("hello world".repeat(100));
  const compressed = compress(data);
  assertThrows(
    () => decompress(compressed, data.length + 100),
    Error,
  );
});

Deno.test("decompress truncated data throws", () => {
  const data = new TextEncoder().encode("hello world".repeat(100));
  const compressed = compress(data);
  assertThrows(
    () => decompress(compressed.slice(0, compressed.length / 2), data.length),
    Error,
  );
});

Deno.test("decompress corrupted data throws", () => {
  const data = new TextEncoder().encode("hello world".repeat(100));
  const compressed = compress(data);
  const corrupted = new Uint8Array(compressed);
  corrupted[0] = 0xff;
  assertThrows(
    () => decompress(corrupted, data.length),
    Error,
  );
});

Deno.test("decompress garbage throws", () => {
  assertThrows(
    () => decompress(new Uint8Array([0, 1, 2, 3, 4, 5]), 100),
    Error,
  );
});
