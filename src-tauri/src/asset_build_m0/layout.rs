use crate::asset_build_m0::diagnostics::DiagnosticSet;
use crate::asset_build_m0::planner::BuildPlan;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildLayout {
    pub root: PathBuf,
    pub build_dir: PathBuf,
    pub plan_dir: PathBuf,
    pub artifact_dir: PathBuf,
    pub package_dir: PathBuf,
    pub report_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildRecord {
    pub build_hash: String,
    pub plan: String,
    pub intermediate_model: String,
    pub diagnostics: String,
}

impl BuildLayout {
    pub fn new(project_root: impl AsRef<Path>, build_hash: &str) -> Result<Self, String> {
        if build_hash.len() != 64 || !build_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("build hash must be a SHA-256 hex string".into());
        }
        let root = project_root.as_ref().to_path_buf();
        let build_dir = root.join(".build").join(build_hash);
        Ok(Self {
            root,
            plan_dir: build_dir.join("plan"),
            artifact_dir: build_dir.join("artifacts"),
            package_dir: build_dir.join("packages"),
            report_dir: build_dir.join("reports"),
            build_dir,
        })
    }

    pub fn write_plan(&self, plan: &BuildPlan) -> Result<BuildRecord, String> {
        for directory in [
            &self.plan_dir,
            &self.artifact_dir,
            &self.package_dir,
            &self.report_dir,
        ] {
            fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        }
        let plan_path = self.plan_dir.join("plan.json");
        let im_path = self.plan_dir.join("im.json");
        write_json(&plan_path, &plan.manifest)?;
        write_json(&im_path, &plan.intermediate)?;
        self.write_diagnostics(&plan.diagnostics)?;
        let diagnostics_path = self.report_dir.join("diagnostics.json");
        let record = BuildRecord {
            build_hash: plan.manifest.build_hash.clone(),
            plan: relative(&self.root, &plan_path),
            intermediate_model: relative(&self.root, &im_path),
            diagnostics: relative(&self.root, &diagnostics_path),
        };
        write_json(&self.build_dir.join("build.json"), &record)?;
        Ok(record)
    }

    pub fn write_diagnostics(&self, diagnostics: &DiagnosticSet) -> Result<(), String> {
        fs::create_dir_all(&self.report_dir).map_err(|error| error.to_string())?;
        write_json(&self.report_dir.join("diagnostics.json"), diagnostics)
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    crate::atomic_fs::write_atomic(path, &bytes, true).map_err(|error| error.to_string())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m0::{lower_schema, parse_schema, plan_build};

    #[test]
    fn writes_m0_artifacts_under_dot_build_without_touching_build() {
        let root = std::env::temp_dir().join(format!("openscp-m0-layout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let raw = serde_json::json!({
            "$schema": "openscp.asset", "schema_version": 2,
            "id": "asset", "kind": "building-asset",
            "reference": {}, "scene": {"document": "asset.lot.json"},
            "outputs": {"package": "asset.package"}
        });
        let parsed = parse_schema(&raw.to_string()).unwrap();
        let plan = plan_build(lower_schema(&parsed).unwrap()).unwrap();
        let layout = BuildLayout::new(&root, &plan.manifest.build_hash).unwrap();
        let record = layout.write_plan(&plan).unwrap();
        assert!(root.join(record.plan).is_file());
        assert!(root.join(".build").is_dir());
        assert!(!root.join("build").exists());
        let _ = fs::remove_dir_all(root);
    }
}
