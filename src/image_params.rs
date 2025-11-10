#[cfg(feature = "ssr")]
use serde::Deserialize;

#[cfg(feature = "ssr")]
#[derive(Deserialize)]
pub struct ImageParams {
    #[serde(default)]
    pub quality: Option<u8>,
    #[serde(default)]
    pub width: Option<u32>,
}

#[cfg(feature = "ssr")]
impl ImageParams {
    pub fn get_valid_presets() -> Vec<(u32, u8)> {
        // Check for environment variable override
        // Format: "1200,80;2400,100;3600,100"
        if let Ok(env_presets) = std::env::var("IMAGE_PRESETS") {
            let mut presets = Vec::new();
            for pair in env_presets.split(';') {
                let parts: Vec<&str> = pair.split(',').collect();
                if parts.len() == 2 {
                    if let (Ok(width), Ok(quality)) = (
                        parts[0].trim().parse::<u32>(),
                        parts[1].trim().parse::<u8>(),
                    ) {
                        presets.push((width, quality));
                    }
                }
            }
            if !presets.is_empty() {
                return presets;
            }
        }

        // Default valid combinations: (width, quality)
        vec![(1200, 80), (2400, 80), (2400, 100), (3600, 100)]
    }

    pub fn validate(&self) -> Result<(u32, u8), &'static str> {
        let valid_presets = Self::get_valid_presets();

        match (self.width, self.quality) {
            (Some(w), Some(q)) => {
                // Both specified - must match a valid preset
                if valid_presets.contains(&(w, q)) {
                    Ok((w, q))
                } else {
                    Err(
                        "Invalid width/quality combination. Check IMAGE_PRESETS environment variable or use defaults: 1200px/80, 2400px/80, 2400px/100, 3600px/100",
                    )
                }
            }
            (Some(w), None) => {
                // Only width specified - find matching preset
                valid_presets
                    .iter()
                    .find(|(width, _)| *width == w)
                    .map(|(w, q)| (*w, *q))
                    .ok_or("Invalid width. Check IMAGE_PRESETS environment variable or use defaults: 1200, 2400, 3600. Note: 2400 has both quality 80 and 100 presets.")
            }
            (None, Some(_)) => Err("Quality must be specified with a width"),
            (None, None) => {
                // No parameters - use default (smallest preset)
                valid_presets
                    .first()
                    .copied()
                    .ok_or("No valid presets configured")
            }
        }
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_validate_with_no_params_uses_default() {
        let params = ImageParams {
            width: None,
            quality: None,
        };
        let result = params.validate();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), (1200, 80));
    }

    #[test]
    fn test_validate_with_valid_width_only() {
        let params = ImageParams {
            width: Some(2400),
            quality: None,
        };
        let result = params.validate();
        assert!(result.is_ok());
        // Should return the first matching preset for 2400, which is now (2400, 80)
        assert_eq!(result.unwrap(), (2400, 80));
    }

    #[test]
    fn test_validate_with_invalid_width_only() {
        let params = ImageParams {
            width: Some(1500),
            quality: None,
        };
        let result = params.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_with_quality_only_fails() {
        let params = ImageParams {
            width: None,
            quality: Some(90),
        };
        let result = params.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_with_valid_combination() {
        let params = ImageParams {
            width: Some(3600),
            quality: Some(100),
        };
        let result = params.validate();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), (3600, 100));
    }

    #[test]
    fn test_validate_with_2400_80_combination() {
        let params = ImageParams {
            width: Some(2400),
            quality: Some(80),
        };
        let result = params.validate();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), (2400, 80));
    }

    #[test]
    fn test_validate_with_2400_100_combination() {
        let params = ImageParams {
            width: Some(2400),
            quality: Some(100),
        };
        let result = params.validate();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), (2400, 100));
    }

    #[test]
    fn test_validate_with_invalid_combination() {
        let params = ImageParams {
            width: Some(1200),
            quality: Some(100),
        };
        let result = params.validate();
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_env_override_valid() {
        std::env::set_var("IMAGE_PRESETS", "800,70;1600,90");
        let presets = ImageParams::get_valid_presets();
        std::env::remove_var("IMAGE_PRESETS");

        assert_eq!(presets.len(), 2);
        assert!(presets.contains(&(800, 70)));
        assert!(presets.contains(&(1600, 90)));
    }

    #[test]
    #[serial]
    fn test_env_override_with_spaces() {
        std::env::set_var("IMAGE_PRESETS", "800, 70 ; 1600 , 90");
        let presets = ImageParams::get_valid_presets();
        std::env::remove_var("IMAGE_PRESETS");

        assert_eq!(presets.len(), 2);
        assert!(presets.contains(&(800, 70)));
        assert!(presets.contains(&(1600, 90)));
    }

    #[test]
    #[serial]
    fn test_env_override_invalid_falls_back_to_defaults() {
        std::env::set_var("IMAGE_PRESETS", "invalid,data;nonsense");
        let presets = ImageParams::get_valid_presets();
        std::env::remove_var("IMAGE_PRESETS");

        // Should fall back to defaults (now 4 presets)
        assert_eq!(presets.len(), 4);
        assert!(presets.contains(&(1200, 80)));
        assert!(presets.contains(&(2400, 80)));
        assert!(presets.contains(&(2400, 100)));
        assert!(presets.contains(&(3600, 100)));
    }

    #[test]
    #[serial]
    fn test_env_override_partial_invalid() {
        std::env::set_var("IMAGE_PRESETS", "800,70;invalid;1600,90");
        let presets = ImageParams::get_valid_presets();
        std::env::remove_var("IMAGE_PRESETS");

        // Should only include valid pairs
        assert_eq!(presets.len(), 2);
        assert!(presets.contains(&(800, 70)));
        assert!(presets.contains(&(1600, 90)));
    }

    #[test]
    #[serial]
    fn test_validate_with_env_override() {
        std::env::set_var("IMAGE_PRESETS", "800,70;1600,90");

        let params = ImageParams {
            width: Some(800),
            quality: None,
        };
        let result = params.validate();

        std::env::remove_var("IMAGE_PRESETS");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), (800, 70));
    }

    #[test]
    #[serial]
    fn test_default_with_env_override() {
        std::env::set_var("IMAGE_PRESETS", "800,70;1600,90");

        let params = ImageParams {
            width: None,
            quality: None,
        };
        let result = params.validate();

        std::env::remove_var("IMAGE_PRESETS");

        assert!(result.is_ok());
        // Should use first preset from env
        assert_eq!(result.unwrap(), (800, 70));
    }
}
