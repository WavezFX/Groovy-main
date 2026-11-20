// Groovy's light offline voice (round 4A item 1): Piper's "libritts_r medium"
// English voice for sherpa-onnx, its files at a pinned commit of
// csukuangfj/vits-piper-en_US-libritts_r-medium, with their sizes and SHA-256.
// The voice was trained on LibriTTS-R (CC BY 4.0, credited in MODELS.md).
// Only the English parts of espeak-ng's data are kept. Generated from the
// repository's file list.

use super::ModelFile;

pub(super) static FILES: &[ModelFile] = &[
    ModelFile { name: "MODEL_CARD", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/MODEL_CARD", size: 279, sha256: "0ccde6927e5bb4d743f4ea39618a9387ba18cca3351220a8a9cfdbc68b30fcb9" },
    ModelFile { name: "en_US-libritts_r-medium.onnx", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/en_US-libritts_r-medium.onnx", size: 78581047, sha256: "acd9425049d887a4d04681010c3b2ed143898b824e5bc1f2ef6ddaee4b11ad0e" },
    ModelFile { name: "espeak-ng-data/en_dict", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/en_dict", size: 166944, sha256: "71bd330ba8a2e3e8076e631508208ef49449d6147c17b7bd2b4b1e1468292e35" },
    ModelFile { name: "espeak-ng-data/intonations", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/intonations", size: 2040, sha256: "3f8af65fd3eda9759a10f021d61361c120871f463515229c925995c7f90918cc" },
    ModelFile { name: "espeak-ng-data/lang/gmw/en", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/lang/gmw/en", size: 140, sha256: "4605d5330801de3641c6e366d15f129ea1f5ffbce8722642aba01ace07ab9c83" },
    ModelFile { name: "espeak-ng-data/lang/gmw/en-029", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/lang/gmw/en-029", size: 335, sha256: "faeb8cb201056775f733acbd908c66555fe57bf01cb063920f178f389cbf85d3" },
    ModelFile { name: "espeak-ng-data/lang/gmw/en-GB-scotland", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/lang/gmw/en-GB-scotland", size: 295, sha256: "1ce4282c1f4385dbaf0035e799b2ab1ee9e9a3ab4829100594a3047727b52353" },
    ModelFile { name: "espeak-ng-data/lang/gmw/en-GB-x-gbclan", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/lang/gmw/en-GB-x-gbclan", size: 238, sha256: "2040e176f1f7f27bdce81c32d7e2b1662449d54d264afd3c0ed871b8e42b6e46" },
    ModelFile { name: "espeak-ng-data/lang/gmw/en-GB-x-gbcwmd", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/lang/gmw/en-GB-x-gbcwmd", size: 188, sha256: "927a5ab891c65b30b4426b151d1623a01024f6b773a38497ba1c90cd77a95747" },
    ModelFile { name: "espeak-ng-data/lang/gmw/en-GB-x-rp", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/lang/gmw/en-GB-x-rp", size: 249, sha256: "d0625af7f58561b1b8cf96fd7f93eee6553bcb3eadb9020ae0757bf96e5115e5" },
    ModelFile { name: "espeak-ng-data/lang/gmw/en-US", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/lang/gmw/en-US", size: 257, sha256: "41534c2a22df5dd4f1052ff9e1a33a3ea7bff5a26b5c02bdad5ba8ddb7524704" },
    ModelFile { name: "espeak-ng-data/lang/gmw/en-US-nyc", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/lang/gmw/en-US-nyc", size: 271, sha256: "e8ee9168376d1f7d9e7dcda7c4644c36c529a8bea816e6380895364db46ea2e3" },
    ModelFile { name: "espeak-ng-data/phondata", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/phondata", size: 550424, sha256: "4e0288957874029a8c3c9f41a8f517ad4bf18127046decbdd4b9d1d6807ce3a3" },
    ModelFile { name: "espeak-ng-data/phondata-manifest", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/phondata-manifest", size: 21821, sha256: "7b387af0702c7cf0b61f0bead68feded0bd8e1620729b0b252e76acbc30d3813" },
    ModelFile { name: "espeak-ng-data/phonindex", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/phonindex", size: 39074, sha256: "3ca7b8fa3b42624e4b0f152707e7a39245fce569aa99ea47c055d9e622fcf0c4" },
    ModelFile { name: "espeak-ng-data/phontab", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/espeak-ng-data/phontab", size: 55796, sha256: "886f3fa402cb0ba73d483aa8ad000af47a6b7cc06293c75a97913fba68a530f6" },
    ModelFile { name: "tokens.txt", url: "https://huggingface.co/csukuangfj/vits-piper-en_US-libritts_r-medium/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/tokens.txt", size: 954, sha256: "620e1aecf1a68fea3ba5850d137b0138fa2037c9b372dad13b95a2a215d0849a" },
];
