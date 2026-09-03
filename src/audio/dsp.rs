#![allow(dead_code)]

/// Coefficient for a single biquad section.
#[derive(Clone, Copy, Debug)]
pub struct BiquadCoeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct BiquadState {
    z1: f32,
    z2: f32,
}

impl BiquadState {
    pub fn new() -> Self {
        Self { z1: 0.0, z2: 0.0 }
    }

    #[inline]
    pub fn process(&mut self, c: &BiquadCoeffs, x: f32) -> f32 {
        // Direct Form 2 Transposed
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }
}

impl Default for BiquadState {
    fn default() -> Self {
        Self::new()
    }
}

/// Peaking EQ filter coefficients (as used in graphic equalizers).
pub fn peaking(sample_rate: f32, freq: f32, gain_db: f32, q: f32) -> BiquadCoeffs {
    let a = 10f32.powf(gain_db / 40.0);
    let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let alpha = w0.sin() / (2.0 * q);
    let cw = w0.cos();
    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cw;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cw;
    let a2 = 1.0 - alpha / a;
    BiquadCoeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// 10-band graphic equalizer (31, 62, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz).
pub struct Eq10 {
    bands: [BiquadCoeffs; 10],
    states_l: [BiquadState; 10],
    states_r: [BiquadState; 10],
    gains_db: [f32; 10],
    sample_rate: f32,
}

pub const EQ_BAND_FREQS: [f32; 10] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

impl Eq10 {
    pub fn new(sample_rate: f32, gains_db: [f32; 10]) -> Self {
        let mut eq = Self {
            bands: [BiquadCoeffs {
                b0: 1.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            }; 10],
            states_l: std::array::from_fn(|_| BiquadState::new()),
            states_r: std::array::from_fn(|_| BiquadState::new()),
            gains_db,
            sample_rate,
        };
        eq.update_coeffs();
        eq
    }

    pub fn set_gains(&mut self, gains_db: [f32; 10]) {
        self.gains_db = gains_db;
        self.update_coeffs();
    }

    pub fn gains(&self) -> [f32; 10] {
        self.gains_db
    }

    fn update_coeffs(&mut self) {
        for (i, &freq) in EQ_BAND_FREQS.iter().enumerate() {
            let nyquist = self.sample_rate / 2.0;
            let f = freq.min(nyquist * 0.95);
            self.bands[i] = peaking(self.sample_rate, f, self.gains_db[i], 1.2);
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (self.sample_rate - sample_rate).abs() > f32::EPSILON {
            self.sample_rate = sample_rate;
            self.update_coeffs();
        }
    }

    #[inline]
    pub fn process_stereo(&mut self, l: f32, r: f32) -> (f32, f32) {
        let mut lo = l;
        let mut ro = r;
        for i in 0..10 {
            lo = self.states_l[i].process(&self.bands[i], lo);
            ro = self.states_r[i].process(&self.bands[i], ro);
        }
        (lo, ro)
    }
}

/// Simple peak limiter: instant attack, smooth release.
pub struct Limiter {
    threshold: f32,
    release_ms: f32,
    sample_rate: f32,
    gain: f32,
}

impl Limiter {
    pub fn new(sample_rate: f32, threshold_db: f32) -> Self {
        Self {
            threshold: 10f32.powf(threshold_db / 20.0),
            release_ms: 60.0,
            sample_rate,
            gain: 1.0,
        }
    }

    pub fn set_threshold_db(&mut self, threshold_db: f32) {
        self.threshold = 10f32.powf(threshold_db / 20.0);
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs());
        // Attack: clamp immediately when over threshold.
        if peak > self.threshold {
            self.gain = self.threshold / peak.max(1e-9);
        } else {
            // Release: exponential recovery toward unity.
            let release_c = (-1.0 / (self.release_ms * 0.001 * self.sample_rate)).exp();
            self.gain = 1.0 + release_c * (self.gain - 1.0);
        }
        (l * self.gain, r * self.gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_flat_passes_signal() {
        let mut eq = Eq10::new(44_100.0, [0.0; 10]);
        let out = eq.process_stereo(0.5, -0.5);
        assert!(
            (out.0 - 0.5).abs() < 1e-4,
            "flat EQ must be transparent: {out:?}"
        );
        assert!((out.1 + 0.5).abs() < 1e-4);
    }

    #[test]
    fn eq_boosts_band() {
        let mut eq = Eq10::new(
            44_100.0,
            [0.0, 0.0, 0.0, 0.0, 0.0, 12.0, 0.0, 0.0, 0.0, 0.0],
        );
        // 1 kHz sine should be amplified after settling
        let sr = 44_100.0f32;
        let mut max_out: f32 = 0.0;
        for n in 0..44_100 {
            let x = (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / sr).sin() * 0.1;
            let (l, _) = eq.process_stereo(x, 0.0);
            if n > sr as i32 * 6 / 10 {
                max_out = max_out.max(l.abs());
            }
        }
        assert!(max_out > 0.2, "1kHz must be boosted: {max_out}");
    }
    #[test]
    fn limiter_caps_peak() {
        let mut lim = Limiter::new(44_100.0, -3.0);
        let mut max_peak: f32 = 0.0;
        for _ in 0..44_100 {
            let (l, r) = lim.process(0.99, 0.99);
            max_peak = max_peak.max(l.abs()).max(r.abs());
        }
        // -3 dB threshold ~0.708
        assert!(max_peak <= 0.71, "limiter must cap: {max_peak}");
    }

    #[test]
    fn biquad_silent_input_stays_silent() {
        let mut st = BiquadState::new();
        let c = peaking(44_100.0, 1000.0, 6.0, 1.2);
        for _ in 0..100 {
            assert_eq!(st.process(&c, 0.0), 0.0);
        }
    }
}
