use crate::config::Settings;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const FIELDS: &[&str] = &[
    "theme", "accent_rgb", "panel_rgb", "panel_opacity", "panel_blur", "heading_opacity",
    "text_rgb", "muted_text_rgb", "interface_text_scale", "interface_scale",
    "background_opacity", "background_dim", "background_blur", "background_overlay",
    "lyrics_scale", "lyrics_blur_past", "reduced_motion",
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub version: u8,
    pub name: String,
    pub values: Map<String, Value>,
}

pub fn capture(settings: &Settings, name: String) -> Result<Preset> {
    let all = serde_json::to_value(settings)?;
    let preset = Preset { version: 1, name, values: FIELDS.iter()
        .map(|&field| (field.to_owned(), if !field.ends_with("_blur") && let Some(number) = all[field].as_f64() {
            serde_json::json!((number * 1_000_000.0).round() / 1_000_000.0)
        } else { all[field].clone() })).collect() };
    validate(&preset)?;
    Ok(preset)
}

pub fn validate(preset: &Preset) -> Result<()> {
    ensure!(preset.version == 1, "Unsupported theme version");
    ensure!(!preset.name.trim().is_empty() && preset.name.chars().count() <= 64, "Theme name must contain 1–64 characters");
    ensure!(!preset.values.is_empty(), "Theme has no appearance settings");
    for (key, value) in &preset.values {
        ensure!(FIELDS.contains(&key.as_str()), "Unexpected theme field: {key}");
        let mut probe = Map::new();
        probe.insert(key.clone(), value.clone());
        let _: Settings = serde_json::from_value(Value::Object(probe))?;
        let bounds = match key.as_str() {
            "panel_opacity" | "heading_opacity" | "background_overlay" => Some((0.0, 1.0)),
            "background_opacity" => Some((0.0, 0.7)),
            "background_dim" => Some((0.0, 0.85)),
            "panel_blur" => Some((0.0, 40.0)),
            "background_blur" => Some((0.0, 50.0)),
            "interface_text_scale" => Some((0.9, 1.25)),
            "interface_scale" => Some((0.9, 1.15)),
            "lyrics_scale" => Some((0.8, 1.5)),
            _ => None,
        };
        if let Some((min, max)) = bounds {
            if key == "heading_opacity" && value.is_null() { continue; }
            let number = value.as_f64().ok_or_else(|| anyhow::anyhow!("Expected a number"))?;
            ensure!(number.is_finite() && (min..=max).contains(&number), "Theme value is out of range: {key}");
        }
    }
    Ok(())
}

pub fn apply(settings: &Settings, preset: &Preset) -> Result<Settings> {
    validate(preset)?;
    let mut value = serde_json::to_value(settings)?;
    for (key, field) in &preset.values { value[key] = field.clone(); }
    Ok(serde_json::from_value(value)?)
}

pub fn read(path: &std::path::Path) -> Result<Preset> {
    ensure!(std::fs::metadata(path)?.len() <= 32_768, "Theme file is too large");
    let preset: Preset = serde_json::from_slice(&std::fs::read(path)?)?;
    validate(&preset)?;
    Ok(preset)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exports_only_appearance_and_applies_without_losing_session() {
        let mut settings = Settings::default();
        settings.client_id = Some("secret".into());
        settings.last_position_ms = Some(1234);
        let preset = capture(&settings, "My theme".into()).unwrap();
        let json = serde_json::to_string(&preset).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("background_image"));
        assert!(!json.contains("last_position"));
        assert_eq!(apply(&settings, &preset).unwrap().last_position_ms, Some(1234));
    }
    #[test]
    fn rejects_secret_fields_and_excessive_sizes() {
        let mut preset = capture(&Settings::default(), "Theme".into()).unwrap();
        preset.values.insert("client_id".into(), Value::Null);
        assert!(validate(&preset).is_err());
        preset.values.remove("client_id");
        preset.values.insert("interface_scale".into(), serde_json::json!(10));
        assert!(validate(&preset).is_err());
    }

    #[test]
    fn boundary_scales_and_full_blur_round_trip_without_float_errors() {
        let settings = Settings { interface_text_scale: 0.9, interface_scale: 1.15, lyrics_scale: 0.8,
            background_blur: 50, panel_blur: 40, ..Default::default() };
        let preset = capture(&settings, "Boundaries".into()).unwrap();
        let restored = apply(&settings, &preset).unwrap();
        assert_eq!(restored.interface_text_scale, 0.9);
        assert_eq!(restored.background_blur, 50);
    }
}
