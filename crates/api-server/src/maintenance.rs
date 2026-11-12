//! Maintenance of stored templates: re-rendering previews and sprites
//! (admin `rerender`, approvals of templates without sprites) and the
//! start-up [`backfill`] of rows from before schema version 2 (derived
//! device tags overall and per canvas, sample counts, missing sprites of
//! approved templates).

use std::sync::Arc;

use serde_json::{Value, json};
use vitrine_core::Project;
use vitrine_core::template::{CanvasInfo, Limits, Package, tags};
use vitrine_render::template::{PREVIEW_HEIGHT, add_previews};

use crate::AppState;
use crate::db::{self, APPROVED};
use crate::error::{ApiError, ApiResult};
use crate::sprites;

/// Reads the stored package of a template (checked again).
fn stored_package(state: &AppState, id: &str) -> anyhow::Result<Package> {
    let bytes = state.storage.package(id)?;
    // Stored packages passed the upload limit of their time: read them
    // with the larger of the current upload limit and the client limits.
    let limits =
        Limits::for_package((state.config.max_upload as u64).max(Limits::default().max_package));
    Ok(Package::from_zip_bytes(&bytes, limits)?)
}

/// The manifest's canvases as the API stores them: each with the derived
/// `devices` of that canvas alone.
pub(crate) fn canvases_json(project: &Project, canvases: &[CanvasInfo]) -> Value {
    Value::Array(
        canvases
            .iter()
            .map(|c| {
                let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
                v["devices"] = json!(tags::devices_of(project, &[c.id.as_str()]));
                v
            })
            .collect(),
    )
}

/// Renders the sprites of a stored template again (and its previews when
/// `previews`), stores them and returns the new sprite metadata. Waits for
/// a render slot; the work runs in a blocking task.
pub async fn rerender(state: &Arc<AppState>, id: &str, previews: bool) -> ApiResult<Value> {
    let _slot = state.render_slot().await;
    let st = state.clone();
    let id_owned = id.to_owned();
    let list = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let mut pkg = stored_package(&st, &id_owned)?;
        if previews {
            add_previews(&mut pkg, PREVIEW_HEIGHT)?;
            st.storage.put(&pkg)?;
        }
        let list = sprites::render(&pkg)?;
        st.storage.put_sprites(&id_owned, &list)?;
        Ok(list)
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(|e| ApiError::internal(format!("cannot render `{id}`: {e:#}")))?;
    let metas = sprites::metas_json(&list);
    db::set_sprites(&state.db(), id, &metas)?;
    Ok(metas)
}

/// Fills what older rows lack: devices, sample counts and facet tags from
/// the stored package, then the sprites of approved templates that have
/// none. Errors are logged and skipped (one broken template never stops
/// the server).
pub async fn backfill(state: Arc<AppState>) {
    let ids = db::not_backfilled(&state.db()).unwrap_or_default();
    for id in ids {
        let st = state.clone();
        let id2 = id.clone();
        let res = tokio::task::spawn_blocking(move || -> anyhow::Result<db::Derived> {
            let pkg = stored_package(&st, &id2)?;
            let project = pkg.project()?;
            Ok(db::Derived {
                devices: json!(tags::devices(&project)),
                samples: i64::try_from(pkg.manifest.samples.len()).unwrap_or(0),
                tags: db::tags_json(&pkg.manifest.tags),
                canvases: canvases_json(&project, &pkg.manifest.canvases),
            })
        })
        .await;
        match res {
            Ok(Ok(derived)) => {
                if let Err(e) = db::set_derived(&state.db(), &id, &derived) {
                    eprintln!("backfill `{id}`: {e}");
                }
            }
            Ok(Err(e)) => eprintln!("backfill `{id}`: {e:#}"),
            Err(e) => eprintln!("backfill `{id}`: {e}"),
        }
    }
    let rows = db::all(&state.db()).unwrap_or_default();
    for t in rows {
        let empty = t.sprites.as_array().is_none_or(Vec::is_empty);
        if t.status == APPROVED && empty {
            match rerender(&state, &t.id, false).await {
                Ok(_) => eprintln!("rendered sprites of `{}`", t.id),
                Err(e) => eprintln!("sprites of `{}`: {}", t.id, e.message),
            }
        }
    }
}
