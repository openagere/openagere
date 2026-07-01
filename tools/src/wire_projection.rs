use crate::JsonSchema;
use crate::ResponsesApiNamespaceTool;
use crate::ToolName;
use crate::ToolSpec;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::hash::Hasher;

const MAX_FLAT_WIRE_TOOL_NAME_LEN: usize = 64;
const HASH_SUFFIX_LEN: usize = 8;
const HASH_SEPARATOR: &str = "__";

#[derive(Debug, Clone, PartialEq)]
pub struct FlatWireFunctionTool {
    pub wire_name: String,
    pub canonical_name: ToolName,
    pub source_kind: FlatWireFunctionToolKind,
    pub description: String,
    pub parameters: JsonSchema,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlatWireFunctionToolKind {
    Function,
    NamespaceFunction,
    ToolSearch,
}

#[derive(Debug, Clone)]
pub struct FlatWireToolProjection {
    function_tools: Vec<FlatWireFunctionTool>,
    wire_names_by_canonical_name: HashMap<ToolName, String>,
    wire_names_by_tool_key: HashMap<FlatWireFunctionToolKey, String>,
    canonical_names_by_wire_name: HashMap<String, ToolName>,
    source_kinds_by_wire_name: HashMap<String, FlatWireFunctionToolKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FlatWireFunctionToolKey {
    canonical_name: ToolName,
    source_kind: FlatWireFunctionToolKind,
}

impl FlatWireToolProjection {
    pub fn new(specs: &[ToolSpec]) -> Self {
        let mut projected = specs
            .iter()
            .flat_map(project_function_tools_for_flat_wire_api)
            .collect::<Vec<_>>();
        disambiguate_wire_names(&mut projected);

        let wire_names_by_canonical_name = projected
            .iter()
            .map(|tool| (tool.canonical_name.clone(), tool.wire_name.clone()))
            .collect();
        let wire_names_by_tool_key = projected
            .iter()
            .map(|tool| {
                (
                    FlatWireFunctionToolKey {
                        canonical_name: tool.canonical_name.clone(),
                        source_kind: tool.source_kind,
                    },
                    tool.wire_name.clone(),
                )
            })
            .collect();
        let canonical_names_by_wire_name = projected
            .iter()
            .map(|tool| (tool.wire_name.clone(), tool.canonical_name.clone()))
            .collect();
        let source_kinds_by_wire_name = projected
            .iter()
            .map(|tool| (tool.wire_name.clone(), tool.source_kind))
            .collect();

        Self {
            function_tools: projected,
            wire_names_by_canonical_name,
            wire_names_by_tool_key,
            canonical_names_by_wire_name,
            source_kinds_by_wire_name,
        }
    }

    pub fn function_tools(&self) -> &[FlatWireFunctionTool] {
        &self.function_tools
    }

    pub fn canonical_name_for_wire_name(&self, wire_name: &str) -> ToolName {
        self.canonical_names_by_wire_name
            .get(wire_name)
            .cloned()
            .unwrap_or_else(|| ToolName::plain(wire_name))
    }

    pub fn source_kind_for_wire_name(&self, wire_name: &str) -> Option<FlatWireFunctionToolKind> {
        self.source_kinds_by_wire_name.get(wire_name).copied()
    }

    pub fn wire_name_for_canonical_name(&self, canonical_name: &ToolName) -> String {
        self.wire_names_by_canonical_name
            .get(canonical_name)
            .cloned()
            .unwrap_or_else(|| {
                provider_valid_wire_name(&flattened_tool_name(canonical_name), canonical_name)
            })
    }

    pub fn wire_name_for_function_tool(&self, canonical_name: &ToolName) -> String {
        let source_kind = if canonical_name.namespace.is_some() {
            FlatWireFunctionToolKind::NamespaceFunction
        } else {
            FlatWireFunctionToolKind::Function
        };
        self.wire_name_for_tool_key(canonical_name, source_kind)
    }

    pub fn wire_name_for_tool_search(&self) -> String {
        self.wire_name_for_tool_key(
            &ToolName::plain(crate::TOOL_SEARCH_TOOL_NAME),
            FlatWireFunctionToolKind::ToolSearch,
        )
    }

    fn wire_name_for_tool_key(
        &self,
        canonical_name: &ToolName,
        source_kind: FlatWireFunctionToolKind,
    ) -> String {
        self.wire_names_by_tool_key
            .get(&FlatWireFunctionToolKey {
                canonical_name: canonical_name.clone(),
                source_kind,
            })
            .cloned()
            .unwrap_or_else(|| {
                provider_valid_wire_name(&flattened_tool_name(canonical_name), canonical_name)
            })
    }
}

pub fn project_function_tools_for_flat_wire_api(spec: &ToolSpec) -> Vec<FlatWireFunctionTool> {
    match spec {
        ToolSpec::Function(tool) => vec![FlatWireFunctionTool {
            wire_name: provider_valid_wire_name(&tool.name, &ToolName::plain(tool.name.clone())),
            canonical_name: ToolName::plain(tool.name.clone()),
            source_kind: FlatWireFunctionToolKind::Function,
            description: tool.description.clone(),
            parameters: tool.parameters.clone(),
        }],
        ToolSpec::Namespace(namespace) => namespace
            .tools
            .iter()
            .map(|tool| match tool {
                ResponsesApiNamespaceTool::Function(tool) => {
                    let canonical_name =
                        ToolName::namespaced(namespace.name.clone(), tool.name.clone());
                    FlatWireFunctionTool {
                        wire_name: provider_valid_wire_name(
                            &flattened_tool_name(&canonical_name),
                            &canonical_name,
                        ),
                        canonical_name,
                        source_kind: FlatWireFunctionToolKind::NamespaceFunction,
                        description: tool.description.clone(),
                        parameters: tool.parameters.clone(),
                    }
                }
            })
            .collect(),
        ToolSpec::ToolSearch {
            description,
            parameters,
            ..
        } => vec![FlatWireFunctionTool {
            wire_name: crate::TOOL_SEARCH_TOOL_NAME.to_string(),
            canonical_name: ToolName::plain(crate::TOOL_SEARCH_TOOL_NAME),
            source_kind: FlatWireFunctionToolKind::ToolSearch,
            description: description.clone(),
            parameters: parameters.clone(),
        }],
        ToolSpec::LocalShell {}
        | ToolSpec::ImageGeneration { .. }
        | ToolSpec::WebSearch { .. }
        | ToolSpec::Freeform(_) => Vec::new(),
    }
}

pub fn resolve_flattened_tool_name(specs: &[ToolSpec], wire_name: &str) -> ToolName {
    FlatWireToolProjection::new(specs).canonical_name_for_wire_name(wire_name)
}

fn disambiguate_wire_names(projected: &mut [FlatWireFunctionTool]) {
    let mut tools_by_wire_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, tool) in projected.iter().enumerate() {
        tools_by_wire_name
            .entry(tool.wire_name.clone())
            .or_default()
            .push(idx);
    }

    let mut colliding_indexes = HashSet::new();
    for indexes in tools_by_wire_name.values() {
        if indexes.len() <= 1 {
            continue;
        }

        for idx in indexes {
            colliding_indexes.insert(*idx);
        }
    }

    let mut used_wire_names = HashSet::new();
    for (idx, tool) in projected.iter_mut().enumerate() {
        if !colliding_indexes.contains(&idx) && used_wire_names.insert(tool.wire_name.clone()) {
            continue;
        }

        let base_wire_name = tool.wire_name.clone();
        for attempt in 0.. {
            let canonical_name = canonical_name_for_disambiguation(&tool.canonical_name, attempt);
            let candidate = append_hash_suffix(&base_wire_name, &canonical_name);
            if used_wire_names.insert(candidate.clone()) {
                tool.wire_name = candidate;
                break;
            }
        }
    }
}

fn provider_valid_wire_name(candidate: &str, canonical_name: &ToolName) -> String {
    let sanitized = sanitize_provider_tool_name(candidate);
    if sanitized.len() <= MAX_FLAT_WIRE_TOOL_NAME_LEN {
        sanitized
    } else {
        append_hash_suffix(&sanitized, canonical_name)
    }
}

fn sanitize_provider_tool_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized
    }
}

fn append_hash_suffix(wire_name: &str, canonical_name: &ToolName) -> String {
    let suffix = stable_tool_name_suffix(canonical_name);
    let max_prefix_len = MAX_FLAT_WIRE_TOOL_NAME_LEN - HASH_SEPARATOR.len() - HASH_SUFFIX_LEN;
    let prefix = if wire_name.len() > max_prefix_len {
        &wire_name[..max_prefix_len]
    } else {
        wire_name
    };
    format!("{prefix}{HASH_SEPARATOR}{suffix}")
}

fn stable_tool_name_suffix(tool_name: &ToolName) -> String {
    let mut hasher = StableHasher::default();
    tool_name.namespace.hash(&mut hasher);
    tool_name.name.hash(&mut hasher);
    format!("{:08x}", hasher.finish() as u32)
}

fn canonical_name_for_disambiguation(tool_name: &ToolName, attempt: usize) -> ToolName {
    if attempt == 0 {
        tool_name.clone()
    } else {
        ToolName::new(
            tool_name.namespace.clone(),
            format!("{}#{attempt}", tool_name.name),
        )
    }
}

#[derive(Default)]
struct StableHasher(u64);

impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.0 == 0 {
            self.0 = 0xcbf29ce484222325;
        }
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

fn flattened_tool_name(tool_name: &ToolName) -> String {
    match tool_name.namespace.as_deref() {
        Some(namespace) if namespace.ends_with('_') || tool_name.name.starts_with('_') => {
            format!("{namespace}{}", tool_name.name)
        }
        Some(namespace) => format!("{namespace}_{}", tool_name.name),
        None => tool_name.name.clone(),
    }
}

#[cfg(test)]
#[path = "wire_projection_tests.rs"]
mod tests;
