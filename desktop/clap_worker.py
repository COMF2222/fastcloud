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


class EmbeddingModels:
    def __init__(self, model):
        self.model = model
        self.options = ort.SessionOptions()
        self.options.intra_op_num_threads = 2
        self.options.inter_op_num_threads = 1
        self.options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        # Inference is intermittent, not a server throughput workload. Do not
        # retain the peak tensor arena between songs; weights stay loaded.
        self.options.enable_cpu_mem_arena = False
        self.audio = self.text = self.tokenizer = None
        self.config = self.filters = None

    def session(self, name):
        return ort.InferenceSession(str(self.model / f"onnx/{name}_model_quantized.onnx"),
                                    self.options, providers=["CPUExecutionProvider"])

    def embed(self, request):
        # Request-sized arrays die when this call returns, rather than living
        # in the stdin loop until another song is analysed minutes later.
        if request["kind"] == "audio":
            rate = int(request["rate"])
            raw = np.fromfile(request["path"], dtype="<f4")
            if not 8_000 <= rate <= 192_000 or len(raw) < rate * 5:
                raise ValueError("Audio sample is too short or rate is invalid")
            if self.audio is None:
                with (self.model / "preprocessor_config.json").open(encoding="utf-8") as handle:
                    self.config = json.load(handle)
                self.filters = mel_filter_bank(self.config)
                self.audio = self.session("audio")
            features = audio_features(raw, rate, self.config, self.filters)
            return self.audio.run(None, {"input_features": features})[0]
        if request["kind"] == "text":
            prompts = request["prompts"]
            if not isinstance(prompts, list) or not 0 < len(prompts) <= 128:
                raise ValueError("Expected 1-128 text prompts")
            if self.text is None:
                self.text = self.session("text")
                self.tokenizer = Tokenizer.from_file(str(self.model / "tokenizer.json"))
                self.tokenizer.enable_truncation(max_length=512)
                self.tokenizer.enable_padding(pad_id=1, pad_token="<pad>")
            chunks = []
            for start in range(0, len(prompts), 8):
                ids = np.asarray([item.ids for item in self.tokenizer.encode_batch(prompts[start:start + 8])], dtype=np.int64)
                chunks.append(self.text.run(None, {"input_ids": ids})[0])
            return np.concatenate(chunks, axis=0)
        raise ValueError("Unknown embedding request")


def main():
    models = EmbeddingModels(Path(sys.argv[1]))
    print(json.dumps({"ready": True, "device": "cpu-onnx"}), flush=True)

    for line in sys.stdin:
        try:
            request = json.loads(line)
            print(json.dumps({"vectors": unit_vectors(models.embed(request))}), flush=True)
        except Exception as error:
            print(json.dumps({"error": str(error)}), flush=True)


if __name__ == "__main__":
    main()
