//! Semantic parsing and diffing for Unity text-serialized scene and prefab YAML.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The Unity YAML file kind supported by the semantic lens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum UnityFileKind {
    /// A Unity scene file.
    Scene,
    /// A Unity prefab file.
    Prefab,
}

/// A coarse semantic change kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum UnityChangeKind {
    /// The item was added.
    Added,
    /// The item was removed.
    Removed,
    /// The item was modified.
    Modified,
    /// The item was moved in the hierarchy or reordered.
    Moved,
    /// The item contains children with changes.
    Unchanged,
}

/// A Unity semantic object kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum UnityNodeKind {
    /// A GameObject.
    GameObject,
    /// A Component attached to a GameObject.
    Component,
    /// A property/field on an object or component.
    Property,
    /// A prefab override/modification row.
    PrefabOverride,
    /// A parser warning surfaced in context.
    Warning,
}

/// A 1-based inclusive line range in a Unity YAML file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnityLineRange {
    /// First 1-based line.
    pub start: u32,
    /// Last 1-based line.
    pub end: u32,
}

/// A selectable semantic range. API callers can convert this to concrete diff hunks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnitySemanticSelectionRange {
    /// Old-file range, if the semantic item existed before.
    pub old: Option<UnityLineRange>,
    /// New-file range, if the semantic item exists after.
    pub new: Option<UnityLineRange>,
}

/// A single property-level semantic change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnitySemanticChange {
    /// Human-readable change label.
    pub label: String,
    /// Unity serialized property path.
    pub property_path: String,
    /// Previous value, if any.
    pub old_value: Option<String>,
    /// Current value, if any.
    pub new_value: Option<String>,
    /// Coarse change kind.
    pub change_kind: UnityChangeKind,
    /// Line ranges that contributed to this change.
    pub range: UnitySemanticSelectionRange,
}

/// A node in the semantic Unity change tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnitySemanticNode {
    /// Stable node id for the current diff result.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Node kind.
    pub kind: UnityNodeKind,
    /// Coarse change kind.
    pub change_kind: UnityChangeKind,
    /// Human-readable hierarchy path.
    pub path: String,
    /// Unity class name when known.
    pub class_name: Option<String>,
    /// Child semantic nodes.
    pub children: Vec<UnitySemanticNode>,
    /// Property-level changes attached to this node.
    pub changes: Vec<UnitySemanticChange>,
    /// Line ranges that contributed to this node.
    pub range: UnitySemanticSelectionRange,
}

/// Summary counts for a Unity semantic diff.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnitySemanticSummary {
    /// Changed GameObject count.
    pub objects_changed: usize,
    /// Changed component count.
    pub components_changed: usize,
    /// Changed prefab override count.
    pub prefab_overrides_changed: usize,
    /// Changed serialized property count.
    pub properties_changed: usize,
    /// Parser warning count.
    pub warnings: usize,
}

/// A warning produced while parsing or diffing Unity YAML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnitySemanticWarning {
    /// Human-readable warning message.
    pub message: String,
    /// File line associated with the warning, if known.
    pub line: Option<u32>,
}

/// A semantic Unity scene/prefab diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnitySemanticDiff {
    /// File kind.
    pub file_kind: UnityFileKind,
    /// Summary counts.
    pub summary: UnitySemanticSummary,
    /// Top-level semantic nodes.
    pub nodes: Vec<UnitySemanticNode>,
    /// Warnings produced while parsing/diffing.
    pub warnings: Vec<UnitySemanticWarning>,
    /// Whether a raw diff is available as fallback.
    pub raw_available: bool,
}

/// Returns true for Unity scene/prefab paths supported by the semantic lens.
pub fn is_supported_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".unity") || lower.ends_with(".prefab")
}

/// Infers the supported Unity file kind from a path.
pub fn file_kind(path: &str) -> Option<UnityFileKind> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".unity") {
        Some(UnityFileKind::Scene)
    } else if lower.ends_with(".prefab") {
        Some(UnityFileKind::Prefab)
    } else {
        None
    }
}

/// Produce a semantic diff for supported Unity YAML content.
pub fn semantic_diff(
    path: &str,
    old_content: Option<&str>,
    new_content: Option<&str>,
    raw_available: bool,
) -> Option<UnitySemanticDiff> {
    let file_kind = file_kind(path)?;
    let old_scene = old_content.map(parse_scene).unwrap_or_default();
    let new_scene = new_content.map(parse_scene).unwrap_or_default();

    let mut nodes = diff_scenes(&old_scene, &new_scene);
    nodes.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.label.cmp(&b.label)));

    let mut warnings = Vec::new();
    warnings.extend(old_scene.warnings);
    warnings.extend(new_scene.warnings);

    let mut summary = UnitySemanticSummary {
        warnings: warnings.len(),
        ..UnitySemanticSummary::default()
    };
    summarize_nodes(&nodes, &mut summary);

    Some(UnitySemanticDiff {
        file_kind,
        summary,
        nodes,
        warnings,
        raw_available,
    })
}

#[derive(Debug, Clone, Default)]
struct ParsedScene {
    documents: BTreeMap<i64, UnityDocument>,
    game_objects: BTreeMap<i64, GameObjectInfo>,
    components_by_go: BTreeMap<i64, Vec<i64>>,
    warnings: Vec<UnitySemanticWarning>,
}

#[derive(Debug, Clone)]
struct UnityDocument {
    id: i64,
    class_name: String,
    range: UnityLineRange,
    properties: BTreeMap<String, UnityProperty>,
    raw_lines: Vec<String>,
}

#[derive(Debug, Clone)]
struct UnityProperty {
    path: String,
    value: String,
    range: UnityLineRange,
}

#[derive(Debug, Clone, Default)]
struct GameObjectInfo {
    name: String,
    parent: Option<i64>,
    children: Vec<i64>,
}

fn parse_scene(content: &str) -> ParsedScene {
    let mut scene = ParsedScene::default();
    let lines: Vec<&str> = content.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if !line.starts_with("--- !u!") {
            index += 1;
            continue;
        }

        let start = index;
        let Some((_class_id, id)) = parse_header(line) else {
            scene.warnings.push(UnitySemanticWarning {
                message: "Could not parse Unity YAML document header.".to_owned(),
                line: Some((index + 1) as u32),
            });
            index += 1;
            continue;
        };

        index += 1;
        while index < lines.len() && !lines[index].starts_with("--- !u!") {
            index += 1;
        }
        let end = index.saturating_sub(1);
        let block = &lines[start + 1..=end];
        let class_name = block
            .iter()
            .find_map(|line| line.strip_suffix(':'))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("UnityObject")
            .to_owned();
        let properties = parse_properties(block, (start + 2) as u32);
        let document = UnityDocument {
            id,
            class_name,
            range: UnityLineRange {
                start: (start + 1) as u32,
                end: (end + 1) as u32,
            },
            properties,
            raw_lines: block.iter().map(|line| (*line).to_owned()).collect(),
        };
        scene.documents.insert(id, document);
    }

    index_scene_relationships(&mut scene);
    scene
}

fn parse_header(line: &str) -> Option<(i32, i64)> {
    let rest = line.strip_prefix("--- !u!")?;
    let (class, anchor) = rest.split_once('&')?;
    Some((class.trim().parse().ok()?, anchor.trim().parse().ok()?))
}

fn parse_properties(lines: &[&str], first_line: u32) -> BTreeMap<String, UnityProperty> {
    let mut properties = BTreeMap::new();
    let mut stack: Vec<(usize, String)> = Vec::new();

    for (offset, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('%')
            || trimmed.starts_with("---")
            || !trimmed.contains(':')
        {
            continue;
        }

        let indent = line.chars().take_while(|ch| *ch == ' ').count();
        let normalized = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let Some((key, value)) = normalized.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || key.starts_with('{') || key.starts_with('}') {
            continue;
        }

        while stack.last().is_some_and(|(level, _)| *level >= indent) {
            stack.pop();
        }

        let path = if key == "propertyPath" || key == "value" || key == "objectReference" {
            key.to_owned()
        } else if stack.is_empty() {
            key.to_owned()
        } else {
            let prefix = stack
                .iter()
                .map(|(_, part)| part.as_str())
                .collect::<Vec<_>>()
                .join(".");
            format!("{prefix}.{key}")
        };
        let value = clean_value(value.trim());
        let line_no = first_line + offset as u32;
        if !value.is_empty() {
            properties.insert(
                path.clone(),
                UnityProperty {
                    path: path.clone(),
                    value: value.clone(),
                    range: UnityLineRange {
                        start: line_no,
                        end: line_no,
                    },
                },
            );
            for (inline_key, inline_value) in parse_inline_mapping(&value) {
                let inline_path = format!("{path}.{inline_key}");
                properties.insert(
                    inline_path.clone(),
                    UnityProperty {
                        path: inline_path,
                        value: inline_value,
                        range: UnityLineRange {
                            start: line_no,
                            end: line_no,
                        },
                    },
                );
            }
        }

        if value.is_empty() {
            stack.push((indent, key.to_owned()));
        }
    }

    properties
}

fn clean_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_owned()
}

fn parse_inline_mapping(value: &str) -> Vec<(String, String)> {
    let trimmed = value.trim();
    let Some(inner) = trimmed
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|part| {
            let (key, value) = part.split_once(':')?;
            Some((key.trim().to_owned(), clean_value(value.trim())))
        })
        .collect()
}

fn index_scene_relationships(scene: &mut ParsedScene) {
    let mut transform_to_go = BTreeMap::new();
    let mut transform_parent = BTreeMap::new();

    for document in scene.documents.values() {
        if document.class_name == "GameObject" {
            let name = document
                .properties
                .get("GameObject.m_Name")
                .or_else(|| document.properties.get("m_Name"))
                .map(|property| property.value.clone())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| format!("GameObject {}", document.id));
            scene.game_objects.insert(
                document.id,
                GameObjectInfo {
                    name,
                    parent: None,
                    children: Vec::new(),
                },
            );
        }

        if is_component(document) {
            if let Some(go_id) = document
                .properties
                .values()
                .find(|property| property.path.ends_with("m_GameObject.fileID"))
                .and_then(|property| property.value.parse::<i64>().ok())
            {
                scene
                    .components_by_go
                    .entry(go_id)
                    .or_default()
                    .push(document.id);
                if document.class_name == "Transform" || document.class_name == "RectTransform" {
                    transform_to_go.insert(document.id, go_id);
                    if let Some(parent_transform) = document
                        .properties
                        .values()
                        .find(|property| property.path.ends_with("m_Father.fileID"))
                        .and_then(|property| property.value.parse::<i64>().ok())
                        .filter(|id| *id != 0)
                    {
                        transform_parent.insert(document.id, parent_transform);
                    }
                }
            }
        }
    }

    for (transform_id, parent_transform_id) in transform_parent {
        let Some(go_id) = transform_to_go.get(&transform_id).copied() else {
            continue;
        };
        let Some(parent_go_id) = transform_to_go.get(&parent_transform_id).copied() else {
            continue;
        };
        if let Some(game_object) = scene.game_objects.get_mut(&go_id) {
            game_object.parent = Some(parent_go_id);
        }
        if let Some(parent) = scene.game_objects.get_mut(&parent_go_id) {
            parent.children.push(go_id);
        }
    }
}

fn diff_scenes(old_scene: &ParsedScene, new_scene: &ParsedScene) -> Vec<UnitySemanticNode> {
    let mut nodes = Vec::new();
    let all_go_ids: BTreeSet<i64> = old_scene
        .game_objects
        .keys()
        .chain(new_scene.game_objects.keys())
        .copied()
        .collect();

    for go_id in all_go_ids {
        let old_go = old_scene.game_objects.get(&go_id);
        let new_go = new_scene.game_objects.get(&go_id);
        let Some(node) = diff_game_object(old_scene, new_scene, go_id, old_go, new_go) else {
            continue;
        };
        nodes.push(node);
    }

    let all_prefab_ids: BTreeSet<i64> = old_scene
        .documents
        .iter()
        .filter_map(|(id, doc)| (doc.class_name == "PrefabInstance").then_some(*id))
        .chain(
            new_scene
                .documents
                .iter()
                .filter_map(|(id, doc)| (doc.class_name == "PrefabInstance").then_some(*id)),
        )
        .collect();

    for id in all_prefab_ids {
        let old_doc = old_scene.documents.get(&id);
        let new_doc = new_scene.documents.get(&id);
        let changes = diff_prefab_overrides(old_doc, new_doc);
        if changes.is_empty() {
            continue;
        }
        let label = format!("Prefab overrides {}", id);
        nodes.push(UnitySemanticNode {
            id: format!("prefab-{id}"),
            label: label.clone(),
            kind: UnityNodeKind::PrefabOverride,
            change_kind: summarize_change_kind(&changes),
            path: label,
            class_name: Some("PrefabInstance".to_owned()),
            children: Vec::new(),
            range: range_for_docs(old_doc, new_doc),
            changes,
        });
    }

    nodes
}

fn diff_game_object(
    old_scene: &ParsedScene,
    new_scene: &ParsedScene,
    go_id: i64,
    old_go: Option<&GameObjectInfo>,
    new_go: Option<&GameObjectInfo>,
) -> Option<UnitySemanticNode> {
    let old_doc = old_scene.documents.get(&go_id);
    let new_doc = new_scene.documents.get(&go_id);
    let mut children = Vec::new();
    let mut changes = diff_document_properties(old_doc, new_doc)
        .into_iter()
        .filter(|change| !change.property_path.contains("m_Component"))
        .collect::<Vec<_>>();

    if old_go.and_then(|go| go.parent) != new_go.and_then(|go| go.parent)
        && old_go.is_some()
        && new_go.is_some()
    {
        changes.push(UnitySemanticChange {
            label: "Parent changed".to_owned(),
            property_path: "m_Father".to_owned(),
            old_value: old_go
                .and_then(|go| go.parent)
                .map(|id| object_path(old_scene, id)),
            new_value: new_go
                .and_then(|go| go.parent)
                .map(|id| object_path(new_scene, id)),
            change_kind: UnityChangeKind::Moved,
            range: range_for_docs(old_doc, new_doc),
        });
    }

    let component_ids: BTreeSet<i64> = old_scene
        .components_by_go
        .get(&go_id)
        .into_iter()
        .flatten()
        .chain(new_scene.components_by_go.get(&go_id).into_iter().flatten())
        .copied()
        .collect();

    for component_id in component_ids {
        let old_component = old_scene.documents.get(&component_id);
        let new_component = new_scene.documents.get(&component_id);
        if let Some(component) = diff_component(
            old_scene,
            new_scene,
            component_id,
            old_component,
            new_component,
        ) {
            children.push(component);
        }
    }

    if changes.is_empty() && children.is_empty() && old_go.is_some() && new_go.is_some() {
        return None;
    }

    let label = new_go
        .or(old_go)
        .map(|go| go.name.clone())
        .unwrap_or_else(|| format!("GameObject {go_id}"));
    let path = new_go
        .map(|_| object_path(new_scene, go_id))
        .or_else(|| old_go.map(|_| object_path(old_scene, go_id)))
        .unwrap_or_else(|| label.clone());
    let change_kind = match (old_go, new_go) {
        (None, Some(_)) => UnityChangeKind::Added,
        (Some(_), None) => UnityChangeKind::Removed,
        (Some(_), Some(_))
            if changes
                .iter()
                .any(|change| change.change_kind == UnityChangeKind::Moved) =>
        {
            UnityChangeKind::Moved
        }
        _ if changes.is_empty() => UnityChangeKind::Unchanged,
        _ => UnityChangeKind::Modified,
    };

    Some(UnitySemanticNode {
        id: format!("go-{go_id}"),
        label,
        kind: UnityNodeKind::GameObject,
        change_kind,
        path,
        class_name: Some("GameObject".to_owned()),
        children,
        changes,
        range: range_for_docs(old_doc, new_doc),
    })
}

fn diff_component(
    old_scene: &ParsedScene,
    new_scene: &ParsedScene,
    component_id: i64,
    old_doc: Option<&UnityDocument>,
    new_doc: Option<&UnityDocument>,
) -> Option<UnitySemanticNode> {
    let changes = diff_document_properties(old_doc, new_doc)
        .into_iter()
        .filter(|change| !change.property_path.ends_with("m_GameObject.fileID"))
        .collect::<Vec<_>>();
    if changes.is_empty() && old_doc.is_some() && new_doc.is_some() {
        return None;
    }
    let class_name = new_doc.or(old_doc).map(|doc| doc.class_name.clone());
    let label = readable_component_label(new_scene, old_scene, new_doc.or(old_doc));
    let change_kind = match (old_doc, new_doc) {
        (None, Some(_)) => UnityChangeKind::Added,
        (Some(_), None) => UnityChangeKind::Removed,
        _ => UnityChangeKind::Modified,
    };

    Some(UnitySemanticNode {
        id: format!("component-{component_id}"),
        label,
        kind: UnityNodeKind::Component,
        change_kind,
        path: class_name
            .clone()
            .unwrap_or_else(|| format!("Component {component_id}")),
        class_name,
        children: Vec::new(),
        changes,
        range: range_for_docs(old_doc, new_doc),
    })
}

fn diff_document_properties(
    old_doc: Option<&UnityDocument>,
    new_doc: Option<&UnityDocument>,
) -> Vec<UnitySemanticChange> {
    let old_props = old_doc.map(|doc| &doc.properties);
    let new_props = new_doc.map(|doc| &doc.properties);
    let keys: BTreeSet<String> = old_props
        .into_iter()
        .flat_map(|props| props.keys().cloned())
        .chain(
            new_props
                .into_iter()
                .flat_map(|props| props.keys().cloned()),
        )
        .collect();

    let mut changes = Vec::new();
    for key in keys {
        if key.ends_with(".fileID") && !key.contains("m_GameObject") && !key.contains("m_Father") {
            continue;
        }
        let old_property = old_doc.and_then(|doc| doc.properties.get(&key));
        let new_property = new_doc.and_then(|doc| doc.properties.get(&key));
        if old_property.map(|property| &property.value)
            == new_property.map(|property| &property.value)
        {
            continue;
        }

        changes.push(UnitySemanticChange {
            label: readable_property_label(&key),
            property_path: key.clone(),
            old_value: old_property.map(|property| readable_value(&key, &property.value)),
            new_value: new_property.map(|property| readable_value(&key, &property.value)),
            change_kind: match (old_property, new_property) {
                (None, Some(_)) => UnityChangeKind::Added,
                (Some(_), None) => UnityChangeKind::Removed,
                _ => UnityChangeKind::Modified,
            },
            range: UnitySemanticSelectionRange {
                old: old_property.map(|property| property.range),
                new: new_property.map(|property| property.range),
            },
        });
    }
    changes
}

fn diff_prefab_overrides(
    old_doc: Option<&UnityDocument>,
    new_doc: Option<&UnityDocument>,
) -> Vec<UnitySemanticChange> {
    let old_overrides = prefab_overrides(old_doc);
    let new_overrides = prefab_overrides(new_doc);
    let keys: BTreeSet<String> = old_overrides
        .keys()
        .cloned()
        .chain(new_overrides.keys().cloned())
        .collect();
    let mut changes = Vec::new();
    for key in keys {
        let old_property = old_overrides.get(&key);
        let new_property = new_overrides.get(&key);
        if old_property.map(|property| &property.value)
            == new_property.map(|property| &property.value)
        {
            continue;
        }
        changes.push(UnitySemanticChange {
            label: format!("Prefab override {}", readable_property_label(&key)),
            property_path: key.clone(),
            old_value: old_property.map(|property| readable_value(&key, &property.value)),
            new_value: new_property.map(|property| readable_value(&key, &property.value)),
            change_kind: match (old_property, new_property) {
                (None, Some(_)) => UnityChangeKind::Added,
                (Some(_), None) => UnityChangeKind::Removed,
                _ => UnityChangeKind::Modified,
            },
            range: UnitySemanticSelectionRange {
                old: old_property.map(|property| property.range),
                new: new_property.map(|property| property.range),
            },
        });
    }
    changes
}

fn prefab_overrides(doc: Option<&UnityDocument>) -> BTreeMap<String, UnityProperty> {
    let Some(doc) = doc else {
        return BTreeMap::new();
    };
    let mut result = BTreeMap::new();
    let mut current_path: Option<UnityProperty> = None;
    for (offset, line) in doc.raw_lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_no = doc.range.start + offset as u32 + 1;
        if let Some(value) = trimmed.strip_prefix("propertyPath:") {
            let value = clean_value(value);
            current_path = Some(UnityProperty {
                path: value.clone(),
                value,
                range: UnityLineRange {
                    start: line_no,
                    end: line_no,
                },
            });
        } else if let Some(value) = trimmed.strip_prefix("value:")
            && let Some(path) = current_path.take()
        {
            result.insert(
                path.value.clone(),
                UnityProperty {
                    path: path.value,
                    value: clean_value(value),
                    range: UnityLineRange {
                        start: path.range.start,
                        end: line_no,
                    },
                },
            );
        }
    }
    result
}

fn object_path(scene: &ParsedScene, id: i64) -> String {
    let Some(game_object) = scene.game_objects.get(&id) else {
        return format!("GameObject {id}");
    };
    let mut segments = vec![game_object.name.clone()];
    let mut parent = game_object.parent;
    let mut seen = BTreeSet::from([id]);
    while let Some(parent_id) = parent {
        if !seen.insert(parent_id) {
            break;
        }
        let Some(parent_go) = scene.game_objects.get(&parent_id) else {
            break;
        };
        segments.push(parent_go.name.clone());
        parent = parent_go.parent;
    }
    segments.reverse();
    segments.join(" / ")
}

fn is_component(document: &UnityDocument) -> bool {
    document.class_name != "GameObject" && document.class_name != "PrefabInstance"
}

fn readable_component_label(
    new_scene: &ParsedScene,
    old_scene: &ParsedScene,
    document: Option<&UnityDocument>,
) -> String {
    let Some(document) = document else {
        return "Component".to_owned();
    };
    if document.class_name != "MonoBehaviour" {
        return document.class_name.clone();
    }

    document
        .properties
        .get("MonoBehaviour.m_Name")
        .or_else(|| document.properties.get("m_Name"))
        .map(|property| property.value.clone())
        .filter(|value| !value.is_empty())
        .or_else(|| script_guid(document).map(|guid| format!("Script {guid}")))
        .or_else(|| {
            component_owner(new_scene, document.id)
                .or_else(|| component_owner(old_scene, document.id))
                .map(|owner| format!("MonoBehaviour on {owner}"))
        })
        .unwrap_or_else(|| "MonoBehaviour".to_owned())
}

fn component_owner(scene: &ParsedScene, component_id: i64) -> Option<String> {
    scene
        .components_by_go
        .iter()
        .find_map(|(go_id, components)| {
            components
                .contains(&component_id)
                .then(|| object_path(scene, *go_id))
        })
}

fn script_guid(document: &UnityDocument) -> Option<String> {
    document.properties.iter().find_map(|(path, property)| {
        (path.ends_with("m_Script.guid") && !property.value.is_empty())
            .then(|| property.value.clone())
    })
}

fn readable_property_label(path: &str) -> String {
    let label = path
        .rsplit('.')
        .next()
        .unwrap_or(path)
        .trim_start_matches("m_")
        .replace('_', " ");
    match label.as_str() {
        "Name" => "Name".to_owned(),
        "IsActive" => "Active state".to_owned(),
        "Enabled" => "Enabled".to_owned(),
        "LocalPosition" => "Position".to_owned(),
        "LocalRotation" => "Rotation".to_owned(),
        "LocalScale" => "Scale".to_owned(),
        "TagString" => "Tag".to_owned(),
        "Layer" => "Layer".to_owned(),
        "Father" => "Parent".to_owned(),
        _ => label,
    }
}

fn readable_value(path: &str, value: &str) -> String {
    if path.ends_with("fileID") && value == "0" {
        return "None".to_owned();
    }
    if value == "0" && (path.ends_with("m_Enabled") || path.ends_with("m_IsActive")) {
        return "Off".to_owned();
    }
    if value == "1" && (path.ends_with("m_Enabled") || path.ends_with("m_IsActive")) {
        return "On".to_owned();
    }
    value.to_owned()
}

fn range_for_docs(
    old_doc: Option<&UnityDocument>,
    new_doc: Option<&UnityDocument>,
) -> UnitySemanticSelectionRange {
    UnitySemanticSelectionRange {
        old: old_doc.map(|doc| doc.range),
        new: new_doc.map(|doc| doc.range),
    }
}

fn summarize_change_kind(changes: &[UnitySemanticChange]) -> UnityChangeKind {
    if changes
        .iter()
        .all(|change| change.change_kind == UnityChangeKind::Added)
    {
        UnityChangeKind::Added
    } else if changes
        .iter()
        .all(|change| change.change_kind == UnityChangeKind::Removed)
    {
        UnityChangeKind::Removed
    } else {
        UnityChangeKind::Modified
    }
}

fn summarize_nodes(nodes: &[UnitySemanticNode], summary: &mut UnitySemanticSummary) {
    for node in nodes {
        match node.kind {
            UnityNodeKind::GameObject => {
                summary.objects_changed +=
                    usize::from(node.change_kind != UnityChangeKind::Unchanged)
            }
            UnityNodeKind::Component => summary.components_changed += 1,
            UnityNodeKind::PrefabOverride => summary.prefab_overrides_changed += node.changes.len(),
            UnityNodeKind::Property | UnityNodeKind::Warning => {}
        }
        summary.properties_changed += node.changes.len();
        summarize_nodes(&node.children, summary);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEFORE: &str = r"%YAML 1.1
--- !u!1 &100
GameObject:
  m_Component:
  - component: {fileID: 200}
  m_Name: Player
  m_IsActive: 1
--- !u!4 &200
Transform:
  m_GameObject: {fileID: 100}
  m_Father: {fileID: 0}
  m_LocalPosition: {x: 0, y: 1, z: 0}
--- !u!114 &300
MonoBehaviour:
  m_GameObject: {fileID: 100}
  m_Enabled: 1
  m_Name: HealthController
  health: 10
";

    const AFTER: &str = r"%YAML 1.1
--- !u!1 &100
GameObject:
  m_Component:
  - component: {fileID: 200}
  - component: {fileID: 300}
  m_Name: Hero
  m_IsActive: 1
--- !u!4 &200
Transform:
  m_GameObject: {fileID: 100}
  m_Father: {fileID: 0}
  m_LocalPosition: {x: 0, y: 2, z: 0}
--- !u!114 &300
MonoBehaviour:
  m_GameObject: {fileID: 100}
  m_Enabled: 1
  m_Name: HealthController
  health: 25
";

    #[test]
    fn detects_game_object_component_and_property_changes() {
        let diff = semantic_diff("Assets/Scenes/Test.unity", Some(BEFORE), Some(AFTER), true)
            .expect("Unity scene path should be supported");

        assert_eq!(diff.file_kind, UnityFileKind::Scene);
        assert_eq!(diff.summary.objects_changed, 1);
        assert!(diff.summary.components_changed >= 2);
        assert!(diff.summary.properties_changed >= 3);

        let player = diff
            .nodes
            .iter()
            .find(|node| node.label == "Hero")
            .expect("changed GameObject should be labelled by its new Unity name");
        assert!(
            player.changes.iter().any(|change| change.label == "Name"),
            "GameObject rename should be shown as a readable property change"
        );
        assert!(
            player
                .children
                .iter()
                .flat_map(|node| &node.changes)
                .any(|change| change.new_value.as_deref() == Some("25")),
            "component field changes should be attached below the GameObject"
        );
    }

    #[test]
    fn groups_prefab_overrides() {
        let before = r"%YAML 1.1
--- !u!1001 &900
PrefabInstance:
  m_Modification:
    m_Modifications:
    - target: {fileID: 1}
      propertyPath: m_Name
      value: OldName
";
        let after = r"%YAML 1.1
--- !u!1001 &900
PrefabInstance:
  m_Modification:
    m_Modifications:
    - target: {fileID: 1}
      propertyPath: m_Name
      value: NewName
";
        let diff = semantic_diff(
            "Assets/Prefabs/Player.prefab",
            Some(before),
            Some(after),
            true,
        )
        .expect("Unity prefab path should be supported");

        assert_eq!(diff.file_kind, UnityFileKind::Prefab);
        assert_eq!(diff.summary.prefab_overrides_changed, 1);
        assert!(
            diff.nodes
                .iter()
                .any(|node| node.kind == UnityNodeKind::PrefabOverride),
            "prefab modifications should be first-class semantic nodes"
        );
    }
}
