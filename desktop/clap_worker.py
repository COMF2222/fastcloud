"""Offline, CPU-only CLAP worker for the private Fastcloud desktop bundle.

The ONNX weights and this executable are packaged with the installer. Audio is
read from a short-lived local PCM file; no sample or embedding is uploaded.
"""

import json
import os
import sys
from pathlib import Path

os.environ.setdefault("OMP_NUM_THREADS", "2")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "2")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")

import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer
from clap_features import log_mel, mel_filter_bank


def unit_vectors(values):
    values = np.asarray(values, dtype=np.float32)
    values /= np.maximum(np.linalg.norm(values, axis=-1, keepdims=True), 1e-9)
    return np.round(values, 6).tolist()


def audio_features(raw, rate, config, filters):
    target_rate = int(config["sampling_rate"])
    if rate != target_rate:
        count = max(1, round(len(raw) * target_rate / rate))
        raw = np.interp(np.arange(count) * rate / target_rate, np.arange(len(raw)), raw).astype(np.float32)
    maximum = int(config["max_length_s"]) * target_rate
    if len(raw) > maximum:
        raw = raw[:maximum]
    elif len(raw) < maximum:
        raw = np.pad(np.tile(raw, maximum // len(raw)), (0, maximum % len(raw)))
    return log_mel(raw, config, filters)[None, None, :, :]


def main():
    model = Path(sys.argv[1])
    with (model / "preprocessor_config.json").open(encoding="utf-8") as handle:
        config = json.load(handle)
    filters = mel_filter_bank(config)
    options = ort.SessionOptions()
    options.intra_op_num_threads = 2
    options.inter_op_num_threads = 1
    options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
    providers = ["CPUExecutionProvider"]
    audio = ort.InferenceSession(str(model / "onnx/audio_model_quantized.onnx"), options, providers=providers)
    text = None
    tokenizer = None
    print(json.dumps({"ready": True, "device": "cpu-onnx"}), flush=True)

    for line in sys.stdin:
        try:
            request = json.loads(line)
            if request["kind"] == "audio":
                raw = np.fromfile(request["path"], dtype="<f4")
                rate = int(request["rate"])
                if rate < 8_000 or len(raw) < rate * 5:
                    raise ValueError("Audio sample is too short")
                features = audio_features(raw, rate, config, filters)
                vectors = audio.run(None, {"input_features": features})[0]
            elif request["kind"] == "text":
                prompts = request["prompts"]
                if not isinstance(prompts, list) or not 0 < len(prompts) <= 128:
                    raise ValueError("Expected 1-128 text prompts")
                if text is None:
                    text = ort.InferenceSession(str(model / "onnx/text_model_quantized.onnx"), options, providers=providers)
                    tokenizer = Tokenizer.from_file(str(model / "tokenizer.json"))
                    tokenizer.enable_truncation(max_length=512)
                    tokenizer.enable_padding(pad_id=1, pad_token="<pad>")
                chunks = []
                for start in range(0, len(prompts), 8):
                    ids = np.asarray([item.ids for item in tokenizer.encode_batch(prompts[start:start + 8])], dtype=np.int64)
                    chunks.append(text.run(None, {"input_ids": ids})[0])
                vectors = np.concatenate(chunks, axis=0)
            else:
                raise ValueError("Unknown embedding request")
            print(json.dumps({"vectors": unit_vectors(vectors)}), flush=True)
        except Exception as error:
            print(json.dumps({"error": str(error)}), flush=True)


if __name__ == "__main__":
    main()
