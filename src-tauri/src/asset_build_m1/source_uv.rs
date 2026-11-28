//! Packager A21E3516: prepare_asset.py:15-22 and
//! compile_geometry_contract.py:49-54. Keep native UVs; adapt Blender once.
//! Applied only to Rust-native .blend imports per the user's scope directive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum SourceUv {
    Native,
    Blender,
}

impl SourceUv {
    pub(super) fn from_diagnostics(text: &str) -> Result<Self, String> {
        let diagnostic: serde_json::Value = if text.is_empty() {
            serde_json::json!({})
        } else {
            // Unrecognized legacy metadata must retain its previous adapter.
            serde_json::from_str(text).unwrap_or(serde_json::Value::Null)
        };
        // Scope this migration to Rust-native .blend imports. All other model
        // sources retain their existing Building40 adapter byte for byte.
        let rust_native = diagnostic["schema"] == "openscp.ny-native/1"
            && diagnostic["source"] == "OpenSCP native Rust";
        if !rust_native {
            return Ok(Self::Blender);
        }
        match diagnostic
            .get("uvConvention")
            .filter(|value| !value.is_null())
        {
            Some(value) if value == "native-v-down" => Ok(Self::Native),
            Some(value) if value == "blender-v-up" => Ok(Self::Blender),
            Some(_) => Err("Unknown source UV convention".into()),
            // Historical Rust exports already invert overlay V in export.rs.
            None => Ok(Self::Native),
        }
    }

    pub(super) fn adapt(self, uv: [f32; 2]) -> [f32; 2] {
        match self {
            Self::Native => uv,
            Self::Blender => [uv[0], 1.0 - uv[1]],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_native_import_preserves_top_down_uv_including_legacy_exports() {
        for convention in [None, Some("native-v-down")] {
            let mut diagnostic =
                serde_json::json!({"schema":"openscp.ny-native/1", "source":"OpenSCP native Rust"});
            if let Some(value) = convention {
                diagnostic["uvConvention"] = value.into();
            }
            let adapter = SourceUv::from_diagnostics(&diagnostic.to_string()).unwrap();
            assert_eq!(adapter.adapt([0.2, 0.125]), [0.2, 0.125]);
            assert_eq!(adapter.adapt([0.4, 1.75]), [0.4, 1.75]);
        }
    }

    #[test]
    fn other_building_sources_keep_historical_vertex_bytes() {
        for diagnostic in [
            serde_json::json!({}),
            serde_json::json!({"schema":"openscp.ny-native/1", "source":"SC5 Asset Editor 1.23.7"}),
            serde_json::json!({"source":"original game building", "uvConvention":"native-v-down"}),
            serde_json::json!({"schema":"other", "source":"OpenSCP native Rust"}),
        ] {
            let adapter = SourceUv::from_diagnostics(&diagnostic.to_string()).unwrap();
            for uv in [[0.2, 0.125], [0.4, 1.75], [-2.0, -0.5]] {
                assert_eq!(
                    adapter.adapt(uv).map(f32::to_bits),
                    [uv[0], 1.0 - uv[1]].map(f32::to_bits)
                );
            }
        }
    }

    #[test]
    fn rust_native_explicit_convention_is_validated() {
        let mut diagnostic = serde_json::json!({"schema":"openscp.ny-native/1", "source":"OpenSCP native Rust", "uvConvention":"blender-v-up"});
        assert_eq!(
            SourceUv::from_diagnostics(&diagnostic.to_string())
                .unwrap()
                .adapt([0.2, 0.125]),
            [0.2, 0.875]
        );
        diagnostic["uvConvention"] = "unknown".into();
        assert!(SourceUv::from_diagnostics(&diagnostic.to_string()).is_err());
    }
}
