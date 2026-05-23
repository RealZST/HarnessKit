use crate::adapter::{AgentAdapter, ProjectMarker};

/// Agents whose project skill dirs overlap with `agent_name`'s WRITE dirs
/// and which appear to be configured for this project (filtered by exclusive
/// marker presence). Used to warn the user that unsync will also drop these
/// agents' view of the shared skills. Sorted by adapter order; excludes
/// `agent_name` itself.
pub fn shared_skill_dir_agents(
    adapters: &[Box<dyn AgentAdapter>],
    agent_name: &str,
    project_path: &str,
) -> Vec<String> {
    let my_write_dirs: std::collections::HashSet<String> = adapters
        .iter()
        .find(|a| a.name() == agent_name)
        .map(|a| {
            a.project_skill_dirs()
                .into_iter()
                .filter(|p| !p.contains('*'))
                .collect()
        })
        .unwrap_or_default();
    if my_write_dirs.is_empty() {
        return Vec::new();
    }
    let project = std::path::Path::new(project_path);
    let mut out = Vec::new();
    for other in adapters {
        if other.name() == agent_name {
            continue;
        }
        let other_dirs: std::collections::HashSet<String> = other
            .project_skill_dirs()
            .into_iter()
            .chain(other.project_skill_read_dirs())
            .filter(|p| !p.contains('*'))
            .collect();
        if !other_dirs.iter().any(|d| my_write_dirs.contains(d)) {
            continue;
        }
        // Require at least one EXCLUSIVE marker (not in my write dirs); a
        // marker that only matches a shared path may have been created by
        // my own install.
        let present = other.project_markers().into_iter().any(|m| match m {
            ProjectMarker::Dir(p) => {
                !my_write_dirs.contains(p) && project.join(p).is_dir()
            }
            ProjectMarker::File(p) => project.join(p).is_file(),
        });
        if !present {
            continue;
        }
        out.push(other.name().to_string());
    }
    out
}
