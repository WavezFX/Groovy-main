// Whisper for sherpa-onnx (round 3F item 5): OpenAI's Whisper models (MIT),
// exported to ONNX and quantized to int8 by k2-fsa, at pinned commits of
// csukuangfj/sherpa-onnx-whisper-*, with their sizes and SHA-256.

use super::{ModelFile, ModelSet};

pub static WHISPER_BASE: ModelSet = ModelSet {
    id: "whisper-base",
    files: &[
        ModelFile { name: "base-decoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base/resolve/bb53ee204431c90d314c1cc08d28d23e5b7927cc/base-decoder.int8.onnx", size: 130672026, sha256: "9759d217388a01b3a4c7c15533201067b48ae819c4daafc8624e64b9409dc02d" },
        ModelFile { name: "base-encoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base/resolve/bb53ee204431c90d314c1cc08d28d23e5b7927cc/base-encoder.int8.onnx", size: 29120534, sha256: "0b8fb1304b6109976038efff5ace81720e00386f3ff6b54ee8c75291ca0a1e11" },
        ModelFile { name: "base-tokens.txt", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base/resolve/bb53ee204431c90d314c1cc08d28d23e5b7927cc/base-tokens.txt", size: 816730, sha256: "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126" },
    ],
};

pub static WHISPER_BASE_EN: ModelSet = ModelSet {
    id: "whisper-base.en",
    files: &[
        ModelFile { name: "base.en-decoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-decoder.int8.onnx", size: 130669978, sha256: "f7162ad6db2dbef16cfaeaa7f945b9d7dd9c1b8d472f6aca82f2273d185e4d41" },
        ModelFile { name: "base.en-encoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-encoder.int8.onnx", size: 29120534, sha256: "ef6b936f4c9b1d90a3b68634b60c4ed8576b26172b33c2535ec0e933c9edb823" },
        ModelFile { name: "base.en-tokens.txt", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-tokens.txt", size: 835554, sha256: "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930" },
    ],
};

pub static WHISPER_TINY: ModelSet = ModelSet {
    id: "whisper-tiny",
    files: &[
        ModelFile { name: "tiny-decoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny/resolve/65176e2deb88badc814a94058666cadccc29b61c/tiny-decoder.int8.onnx", size: 89855401, sha256: "d2fece8dd42771f1df975c6c0445770d0c292bf7547c2cae04a6c0cc57540925" },
        ModelFile { name: "tiny-encoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny/resolve/65176e2deb88badc814a94058666cadccc29b61c/tiny-encoder.int8.onnx", size: 12937772, sha256: "d24fb083ae3b1041fc24e97971d60e280c9342201fbb67b0ab428a8b4a51a434" },
        ModelFile { name: "tiny-tokens.txt", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny/resolve/65176e2deb88badc814a94058666cadccc29b61c/tiny-tokens.txt", size: 816730, sha256: "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126" },
    ],
};

pub static WHISPER_TINY_EN: ModelSet = ModelSet {
    id: "whisper-tiny.en",
    files: &[
        ModelFile { name: "tiny.en-decoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-decoder.int8.onnx", size: 89853865, sha256: "06c0e6ff6348d427e51839219d1c886c18cfdf411e629e33f5e1679bff9c1527" },
        ModelFile { name: "tiny.en-encoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-encoder.int8.onnx", size: 12937772, sha256: "0ce578b827c94a961aacb8fa14b02f096504b337e5c94be37c36238cbe3e8bc6" },
        ModelFile { name: "tiny.en-tokens.txt", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-tokens.txt", size: 835554, sha256: "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930" },
    ],
};

pub static WHISPER_SMALL: ModelSet = ModelSet {
    id: "whisper-small",
    files: &[
        ModelFile { name: "small-decoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-small/resolve/8f3c18b358db4d1f2fc1eae49d75cd20989e4309/small-decoder.int8.onnx", size: 262226114, sha256: "acad50b5c782696e91b55914cc5ab4f756f1532f76e22aa6fc615f39fb69a8ee" },
        ModelFile { name: "small-encoder.int8.onnx", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-small/resolve/8f3c18b358db4d1f2fc1eae49d75cd20989e4309/small-encoder.int8.onnx", size: 112442483, sha256: "4cbe7b22fa9026b843b60a68640c747de05bafb1a11b57edc0e66c232d9f33a9" },
        ModelFile { name: "small-tokens.txt", url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-small/resolve/8f3c18b358db4d1f2fc1eae49d75cd20989e4309/small-tokens.txt", size: 816730, sha256: "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126" },
    ],
};

/// The set for a model Groovy offers ("base", "tiny.en"…).
pub fn set(model: &str) -> Option<&'static ModelSet> {
    Some(match model {
        "base" => &WHISPER_BASE,
        "base.en" => &WHISPER_BASE_EN,
        "tiny" => &WHISPER_TINY,
        "tiny.en" => &WHISPER_TINY_EN,
        "small" => &WHISPER_SMALL,
        _ => return None,
    })
}
