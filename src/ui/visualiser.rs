/// Which trace the Winamp mini player shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    #[default]
    Spectrum,
    Scope,
    Off,
}

impl Mode {
    pub fn next(self) -> Self {
        match self {
            Self::Spectrum => Self::Scope,
            Self::Scope => Self::Off,
            Self::Off => Self::Spectrum,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Spectrum => "Spectrum",
            Self::Scope => "Oscilloscope",
            Self::Off => "Visualiser off",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mode;

    #[test]
    fn clicking_cycles_the_three_modes() {
        assert_eq!(Mode::default(), Mode::Spectrum);
        assert_eq!(Mode::Spectrum.next(), Mode::Scope);
        assert_eq!(Mode::Scope.next(), Mode::Off);
        assert_eq!(Mode::Off.next(), Mode::Spectrum);
    }
}
