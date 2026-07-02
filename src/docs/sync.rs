use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config::Config;
use crate::docs::frontmatter::LivingDocsMeta;
use crate::docs::manifest::{Manifest, ManifestBlock};
use crate::docs::{managed_blocks, slug, templates, writer};
use crate::graph::queries::{self, ClassSummary};
use crate::synthesis::provider::{Provider, SynthesizeOptions};
use crate::synthesis::{self, Synthesizer};
use crate::util::hash;

#[derive(Debug, Default, Clone, Copy)]
pub struct SyncStats {
    pub files_written: usize,
    pub files_unchanged: usize,
    pub blocks_synthesized: usize,
    pub blocks_reused: usize,
}

/// Regenerate every managed section from the graph. A file whose managed
/// content wouldn't actually change is left completely untouched — front
/// matter included — which is what makes re-running `sync` on an
/// unchanged repo a true no-op (§0's "byte-identical Markdown out").
pub async fn run_sync<P: Provider>(
    conn: &Connection,
    root: &Path,
    config: &Config,
    manifest: &mut Manifest,
    synthesizer: &mut Synthesizer<P>,
) -> Result<SyncStats> {
    let docs_dir = root.join(&config.docs);
    let mut stats = SyncStats::default();

    let classes = queries::list_classes(conn)?;
    let exported: Vec<&ClassSummary> = classes.iter().filter(|c| c.exported).collect();

    let mut component_slugs: Vec<String> = Vec::new();
    for class in &exported {
        let component_slug = slug::slugify(&class.name);
        component_slugs.push(component_slug.clone());
        sync_component(
            conn,
            root,
            &docs_dir,
            manifest,
            synthesizer,
            &config.model,
            class,
            &component_slug,
            &mut stats,
        )
        .await?;
    }
    component_slugs.sort();

    sync_static_file(
        root,
        &docs_dir.join("overview.md"),
        "overview",
        &[(
            templates::overview::BLOCK_ID.to_string(),
            templates::overview::render(conn)?,
        )],
        manifest,
        &mut stats,
    )?;

    sync_static_file(
        root,
        &docs_dir.join("architecture.md"),
        "architecture",
        &[(
            templates::architecture::BLOCK_ID.to_string(),
            templates::architecture::render(conn)?,
        )],
        manifest,
        &mut stats,
    )?;

    sync_static_file(
        root,
        &docs_dir.join("dependencies.md"),
        "dependencies",
        &[(
            templates::dependencies::BLOCK_ID.to_string(),
            templates::dependencies::render(conn)?,
        )],
        manifest,
        &mut stats,
    )?;

    sync_static_file(
        root,
        &docs_dir.join("diagrams").join("component.md"),
        "diagram",
        &[(
            templates::diagrams::BLOCK_ID.to_string(),
            templates::diagrams::render_standalone(conn)?,
        )],
        manifest,
        &mut stats,
    )?;

    let routes = queries::list_routes(conn)?;
    let has_routes = !routes.is_empty();
    let grouped = templates::apis::group_by_resource(&routes);
    let resources: Vec<String> = grouped.keys().cloned().collect();

    if has_routes {
        sync_static_file(
            root,
            &docs_dir.join("apis").join("index.md"),
            "api",
            &[(
                templates::apis::INDEX_BLOCK_ID.to_string(),
                templates::apis::render_index(&routes),
            )],
            manifest,
            &mut stats,
        )?;

        for (resource, resource_routes) in &grouped {
            sync_static_file(
                root,
                &docs_dir.join("apis").join(format!("{resource}.md")),
                "api",
                &[(
                    templates::apis::block_id_for_resource(resource),
                    templates::apis::render_resource(resource_routes),
                )],
                manifest,
                &mut stats,
            )?;
        }
    }

    sync_static_file(
        root,
        &docs_dir.join("index.md"),
        "index",
        &[(
            templates::index::BLOCK_ID.to_string(),
            templates::index::render(&component_slugs, &resources, has_routes),
        )],
        manifest,
        &mut stats,
    )?;

    if stats.files_written > 0 {
        manifest.last_synced = Some(now_iso8601());
        manifest.commit = current_commit(root);
    }

    Ok(stats)
}

/// Write `blocks` into `path` if their content differs from what's on
/// disk; otherwise leave the file untouched. `entity` is always `None`
/// here — only component docs (`sync_component`) have a single owning
/// entity.
fn sync_static_file(
    root: &Path,
    path: &Path,
    kind: &str,
    blocks: &[(String, String)],
    manifest: &mut Manifest,
    stats: &mut SyncStats,
) -> Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let existing_body = body_of(&existing);
    let existing_blocks = managed_blocks::find_blocks(&existing_body);

    let changed = existing.is_empty() || any_block_changed(&existing_blocks, blocks);
    if !changed {
        stats.files_unchanged += 1;
        return Ok(());
    }

    let meta = LivingDocsMeta {
        generated: true,
        kind: kind.to_string(),
        entity: None,
        source: Vec::new(),
        commit: current_commit(root),
        last_synced: Some(now_iso8601()),
        model: None,
    };
    write_file(root, path, &meta, &existing_body, blocks, manifest)?;
    stats.files_written += 1;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn sync_component<P: Provider>(
    conn: &Connection,
    root: &Path,
    docs_dir: &Path,
    manifest: &mut Manifest,
    synthesizer: &mut Synthesizer<P>,
    model_config: &crate::config::ModelConfig,
    class: &ClassSummary,
    component_slug: &str,
    stats: &mut SyncStats,
) -> Result<()> {
    let path = docs_dir.join("components").join(format!("{component_slug}.md"));

    let slice = templates::component::build_slice(conn, class)?;
    let current_facts_hash = hash::stable_id(&serde_json::to_string(&slice)?);

    let responsibilities_id = templates::component::responsibilities_block_id(component_slug);
    let facts_id = templates::component::facts_block_id(component_slug);

    let existing = fs::read_to_string(&path).unwrap_or_default();
    let existing_body = body_of(&existing);
    let existing_blocks = managed_blocks::find_blocks(&existing_body);
    let existing_responsibilities = existing_blocks.iter().find(|b| b.id == responsibilities_id);

    let can_reuse = existing_responsibilities.is_some()
        && manifest
            .blocks
            .get(&responsibilities_id)
            .and_then(|b| b.facts_hash.as_deref())
            == Some(current_facts_hash.as_str());

    let responsibilities_body = if can_reuse {
        stats.blocks_reused += 1;
        existing_responsibilities.unwrap().content.clone()
    } else {
        let prompt = synthesis::prompts::build_prompt(&slice)?;
        let model = synthesis::select_model(model_config, false).to_string();
        let text = synthesizer
            .synthesize(&prompt, &SynthesizeOptions { model, max_tokens: 300 })
            .await?;
        stats.blocks_synthesized += 1;
        text
    };

    let facts_body = templates::component::render_facts(conn, class)?;
    let blocks = [
        (responsibilities_id.clone(), responsibilities_body),
        (facts_id, facts_body),
    ];

    let changed = existing.is_empty() || any_block_changed(&existing_blocks, &blocks);
    if !changed {
        stats.files_unchanged += 1;
        return Ok(());
    }

    let meta = LivingDocsMeta {
        generated: true,
        kind: "component".to_string(),
        entity: Some(class.name.clone()),
        source: vec![class.file.clone()],
        commit: current_commit(root),
        last_synced: Some(now_iso8601()),
        model: Some(synthesis::select_model(model_config, false).to_string()),
    };
    write_file(root, &path, &meta, &existing_body, &blocks, manifest)?;

    // facts_hash only applies to the synthesized block; the deterministic
    // facts block has none — it's always freshly (and cheaply) recomputed.
    if let Some(block) = manifest.blocks.get_mut(&responsibilities_id) {
        block.facts_hash = Some(current_facts_hash);
    }

    stats.files_written += 1;
    Ok(())
}

fn any_block_changed(existing: &[managed_blocks::ManagedBlock], wanted: &[(String, String)]) -> bool {
    wanted.iter().any(|(id, body)| {
        let trimmed = body.trim();
        existing
            .iter()
            .find(|b| &b.id == id)
            .map(|b| b.content != trimmed)
            .unwrap_or(true)
    })
}

fn body_of(content: &str) -> String {
    crate::docs::frontmatter::parse(content)
        .map(|(_, body)| body.to_string())
        .unwrap_or_else(|| content.to_string())
}

fn write_file(
    root: &Path,
    path: &Path,
    meta: &LivingDocsMeta,
    existing_body: &str,
    blocks: &[(String, String)],
    manifest: &mut Manifest,
) -> Result<()> {
    let front_matter = templates::render_frontmatter(meta)?;
    let new_body = writer::upsert_blocks(existing_body, blocks);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, format!("{front_matter}\n{new_body}"))
        .with_context(|| format!("failed to write {}", path.display()))?;

    let file_rel = root
        .canonicalize()
        .ok()
        .and_then(|r| path.canonicalize().ok().map(|p| (r, p)))
        .and_then(|(r, p)| p.strip_prefix(&r).ok().map(|rel| rel.to_path_buf()))
        .unwrap_or_else(|| path.strip_prefix(root).unwrap_or(path).to_path_buf())
        .to_string_lossy()
        .replace('\\', "/");

    for (id, body) in blocks {
        manifest.blocks.insert(
            id.clone(),
            ManifestBlock {
                file: file_rel.clone(),
                entity: meta.entity.clone(),
                source: meta.source.clone(),
                hash: hash::stable_id(body.trim()),
                facts_hash: None,
            },
        );
    }

    Ok(())
}

/// Best-effort current HEAD commit SHA, `None` if this isn't a git repo
/// or has no commits yet — docs generation shouldn't hard-fail over it.
fn current_commit(root: &Path) -> Option<String> {
    let repo = git2::Repository::open(root).ok()?;
    let commit = repo.head().ok()?.peel_to_commit().ok()?;
    Some(commit.id().to_string())
}

/// Current UTC time as `YYYY-MM-DDTHH:MM:SSZ`, computed from a Unix
/// timestamp with no external date/time crate (none is in the locked
/// stack — see PLAN.md §1). `civil_from_days` is Howard Hinnant's
/// well-known days-since-epoch -> Gregorian-date algorithm.
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (time_of_day / 3600, (time_of_day % 3600) / 60, time_of_day % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2000-03-01 is a well-known reference point for this algorithm.
        assert_eq!(civil_from_days(11_017), (2000, 3, 1));
    }
}
