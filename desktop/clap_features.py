"""NumPy CLAP log-mel preprocessing, adapted from Transformers (Apache-2.0).

Only the Slaney, 10-second path used by our LAION music/speech checkpoint is
implemented. Keeping it here avoids shipping PyTorch or the full Transformers
package with the desktop worker.
"""

import numpy as np


def mel_filter_bank(config):
    rate = int(config["sampling_rate"])
    fft_size = int(config["fft_window_size"])
    count = int(config["feature_size"])
    low = float(config["frequency_min"])
    high = float(config["frequency_max"])

    def hz_to_mel(hz):
        hz = np.asarray(hz, dtype=np.float64)
        return np.where(hz >= 1000.0, 15.0 + np.log(np.maximum(hz, 1.0) / 1000.0) * 27.0 / np.log(6.4), hz * 3.0 / 200.0)

    def mel_to_hz(mels):
        return np.where(mels >= 15.0, 1000.0 * np.exp((mels - 15.0) * np.log(6.4) / 27.0), 200.0 * mels / 3.0)

    centers = mel_to_hz(np.linspace(hz_to_mel(low), hz_to_mel(high), count + 2))
    bins = np.linspace(0, rate // 2, fft_size // 2 + 1)
    delta = np.diff(centers)
    slopes = centers[None, :] - bins[:, None]
    weights = np.maximum(0.0, np.minimum(-slopes[:, :-2] / delta[:-1], slopes[:, 2:] / delta[1:]))
    weights *= (2.0 / (centers[2:] - centers[:-2]))[None, :]
    return weights


def log_mel(raw, config, filters):
    fft_size = int(config["fft_window_size"])
    hop = int(config["hop_length"])
    padded = np.pad(raw.astype(np.float64), (fft_size // 2, fft_size // 2), mode="reflect")
    frames = np.lib.stride_tricks.sliding_window_view(padded, fft_size)[::hop]
    window = np.hanning(fft_size + 1)[:-1]
    # Transformers rounds the complex FFT output to complex64 before taking
    # power, so keep the same conversion for stable embeddings.
    spectrum = np.fft.rfft(frames * window, axis=1).astype(np.complex64)
    power = np.abs(spectrum, dtype=np.float64) ** 2
    mel = np.maximum(1e-10, filters.T @ power.T)
    return (10.0 * np.log10(mel)).T.astype(np.float32)
