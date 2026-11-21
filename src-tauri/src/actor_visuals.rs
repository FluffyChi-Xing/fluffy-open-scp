//! Lazy actor assembly payload. Called only after the visual-effects switch is enabled.
use crate::{
    activity::{AppState, CommandError},
    package_service::{TgiDto, locate_sim_source_packages},
};
use dbpf::{Package, ResourceId};
use serde::{Deserialize, Serialize};
use tauri::State;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    package_id: u64,
    model: Option<TgiDto>,
    body: Option<usize>,
    head: Option<usize>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
    body_palette: sc_exporter::actor_assets::Pixels,
    clothes_palette: sc_exporter::actor_assets::Pixels,
    body: sc_exporter::actor_assets::ActorPart,
    head: Option<sc_exporter::actor_assets::ActorPart>,
}
#[tauri::command]
pub async fn read_actor_visuals(
    state: State<'_, AppState>,
    request: Request,
) -> Result<Payload, CommandError> {
    let manager = state.packages.clone();
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let current = manager
            .get(request.package_id)
            .map_err(CommandError::from)?;
        let mut sources =
            locate_sim_source_packages(&manager, &store).map_err(CommandError::from)?;
        if let Some(dir) = sources
            .first()
            .and_then(|(_, p)| p.path().parent())
            .map(|p| p.to_path_buf())
        {
            let app = dir.join("SimCity_App.package");
            if app.is_file() {
                sources.push((
                    "SimCity_App.package".into(),
                    Package::open(app).map_err(|e| CommandError::internal(e.to_string()))?,
                ));
            }
        }
        let mut packages = vec![current.as_ref()];
        packages.extend(sources.iter().map(|(_, p)| p));
        let fail = |e: String| CommandError::new("actor_visuals_unavailable", e);
        let (body, head) = match (request.body, request.head) {
            (Some(body), Some(head)) => {
                if body > 2 || head > 19 {
                    return Err(fail("unsupported body/head index".into()));
                }
                let descriptor = ResourceId {
                    type_id: 0x00B1B104,
                    group: 0x40002D00,
                    instance: 0x22706EFA,
                };
                let pkg = packages
                    .iter()
                    .find(|p| p.entry(descriptor).is_some())
                    .ok_or_else(|| fail("sim part descriptor missing".into()))?;
                let data = pkg
                    .read(pkg.entry(descriptor).unwrap())
                    .map_err(|e| fail(e.to_string()))?;
                let props =
                    sc_properties::PropertyFile::parse(&data).map_err(|e| fail(e.to_string()))?;
                let keys = sc_properties::actor_graphics::keys(&props, 0x0BD62577);
                let get = |index: usize| {
                    keys.get(index)
                        .map(|k| ResourceId {
                            type_id: 0x2F4E681B,
                            group: k.group_id,
                            instance: k.instance_id,
                        })
                        .ok_or_else(|| fail("sim part table incomplete".into()))
                };
                (get(body * 3)?, Some(get(9 + head)?))
            }
            (None, None) => (
                request
                    .model
                    .ok_or_else(|| fail("no actor model".into()))?
                    .into(),
                None,
            ),
            _ => return Err(fail("body and head must be selected together".into())),
        };
        Ok(Payload {
            body_palette: sc_exporter::actor_assets::palette(&packages, 0x69C94C5D)
                .map_err(fail)?,
            clothes_palette: sc_exporter::actor_assets::palette(&packages, 0x14C5F13F)
                .map_err(fail)?,
            body: sc_exporter::actor_assets::read_part(&packages, body).map_err(fail)?,
            head: head
                .map(|h| sc_exporter::actor_assets::read_part(&packages, h).map_err(fail))
                .transpose()?,
        })
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
}
