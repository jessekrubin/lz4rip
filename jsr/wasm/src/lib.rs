use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = "compressBound")]
pub fn compress_bound(input_len: usize) -> usize {
    lz4rip::get_maximum_output_size(input_len)
}

#[wasm_bindgen]
pub fn compress(input: &[u8]) -> Vec<u8> {
    lz4rip::compress(input)
}

#[wasm_bindgen]
pub fn decompress(input: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, JsError> {
    let output =
        lz4rip::decompress(input, uncompressed_size).map_err(|e| JsError::new(&format!("{e}")))?;
    validate_exact_size(output, uncompressed_size)
}

enum CompressorInner {
    Plain(lz4rip::block::Compressor),
    Dict(lz4rip::block::DictCompressor),
}

#[wasm_bindgen]
pub struct Compressor {
    inner: CompressorInner,
}

#[wasm_bindgen]
impl Compressor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Compressor {
        Compressor {
            inner: CompressorInner::Plain(lz4rip::block::Compressor::new()),
        }
    }

    #[wasm_bindgen(js_name = "withDict")]
    pub fn with_dict(dict: &[u8]) -> Compressor {
        Compressor {
            inner: CompressorInner::Dict(lz4rip::block::DictCompressor::new(dict)),
        }
    }

    pub fn compress(&mut self, input: &[u8]) -> Vec<u8> {
        match &mut self.inner {
            CompressorInner::Plain(c) => c.compress(input),
            CompressorInner::Dict(c) => c.compress(input),
        }
    }
}

#[wasm_bindgen]
pub struct Decompressor {
    inner: lz4rip::block::Decompressor,
}

#[wasm_bindgen]
impl Decompressor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Decompressor {
        Decompressor {
            inner: lz4rip::block::Decompressor::new(),
        }
    }

    #[wasm_bindgen(js_name = "withDict")]
    pub fn with_dict(dict: &[u8]) -> Decompressor {
        Decompressor {
            inner: lz4rip::block::Decompressor::with_dict(dict),
        }
    }

    pub fn decompress(
        &self,
        input: &[u8],
        uncompressed_size: usize,
    ) -> Result<Vec<u8>, JsError> {
        let output = self
            .inner
            .decompress(input, uncompressed_size)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        validate_exact_size(output, uncompressed_size)
    }
}

#[wasm_bindgen]
pub struct DictTrainer {
    inner: Option<lz4rip::block::DictTrainer>,
}

#[wasm_bindgen]
impl DictTrainer {
    #[wasm_bindgen(constructor)]
    pub fn new(max_dict_size: usize) -> DictTrainer {
        DictTrainer {
            inner: Some(lz4rip::block::DictTrainer::new(max_dict_size)),
        }
    }

    #[wasm_bindgen(js_name = "addSample")]
    pub fn add_sample(&mut self, data: &[u8]) -> Result<(), JsError> {
        let trainer = self
            .inner
            .as_mut()
            .ok_or_else(|| JsError::new("DictTrainer already consumed by train()"))?;
        trainer.add_sample(data);
        Ok(())
    }

    #[wasm_bindgen(js_name = "sampleCount")]
    pub fn sample_count(&self) -> Result<usize, JsError> {
        self.inner
            .as_ref()
            .map(|t| t.sample_count())
            .ok_or_else(|| JsError::new("DictTrainer already consumed by train()"))
    }

    pub fn train(&mut self) -> Result<Vec<u8>, JsError> {
        let trainer = self
            .inner
            .take()
            .ok_or_else(|| JsError::new("DictTrainer already consumed by train()"))?;
        Ok(trainer.train())
    }
}

fn validate_exact_size(output: Vec<u8>, uncompressed_size: usize) -> Result<Vec<u8>, JsError> {
    if output.len() == uncompressed_size {
        Ok(output)
    } else {
        Err(JsError::new(&format!(
            "decompressed size mismatch: expected {uncompressed_size}, got {}",
            output.len()
        )))
    }
}
