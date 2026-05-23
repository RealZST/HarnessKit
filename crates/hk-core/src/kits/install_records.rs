use crate::kits::types::{ProjectInstallRecords, ProjectKitInstallEntry};
use crate::store::Store;
use crate::HkError;
use parking_lot::Mutex;

/// Group `kit_sync_records` into a per-project view of installed Kits.
/// Sync records are the source of truth — a Kit is "installed in a project"
/// iff at least one sync record exists for that `(kit_id, project_path)`.
pub fn list_project_install_records(
    store: &Mutex<Store>,
) -> Result<Vec<ProjectInstallRecords>, HkError> {
    let store = store.lock();
    let syncs = store.list_all_sync_records()?;
    let kits = store.list_kit_rows()?;
    let mut grouped: std::collections::BTreeMap<String, Vec<ProjectKitInstallEntry>> =
        Default::default();
    for (idx, s) in syncs.into_iter().enumerate() {
        let kit_name = kits
            .iter()
            .find(|k| k.id == s.kit_id)
            .map(|k| k.name.clone())
            .unwrap_or_else(|| "(deleted)".into());
        grouped
            .entry(s.project_path.clone())
            .or_default()
            .push(ProjectKitInstallEntry {
                kit_id: s.kit_id,
                kit_name,
                agent_name: s.agent_name,
                // Sync records come back DESC by synced_at, so the loop index
                // gives newer installs a lower position (= appear first).
                position: idx as i64,
                last_synced_at: Some(s.synced_at),
            });
    }
    Ok(grouped
        .into_iter()
        .map(|(project_path, entries)| ProjectInstallRecords {
            project_path,
            entries,
        })
        .collect())
}
