use crate::config::{write_json_file, write_text_file};
use crate::error::AppError;
use crate::provider::OpenCodeProviderConfig;
use crate::settings::get_opencode_override_dir;
use indexmap::IndexMap;
use jsonc_parser::cst::{CstInputValue, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

pub const OPENCODE_GO_PROVIDER_ID: &str = "opencode-go";
pub const OPENCODE_GO_BASE_URL: &str = "https://opencode.ai/zen/go/v1";

const STANDARD_OMO_PLUGIN_PREFIXES: [&str; 2] = ["oh-my-openagent", "oh-my-opencode"];
const SLIM_OMO_PLUGIN_PREFIXES: [&str; 1] = ["oh-my-opencode-slim"];

fn matches_plugin_prefix(plugin_name: &str, prefix: &str) -> bool {
    plugin_name == prefix
        || plugin_name
            .strip_prefix(prefix)
            .map(|suffix| suffix.starts_with('@'))
            .unwrap_or(false)
}

fn matches_any_plugin_prefix(plugin_name: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|prefix| matches_plugin_prefix(plugin_name, prefix))
}

fn canonicalize_plugin_name(plugin_name: &str) -> String {
    if let Some(suffix) = plugin_name.strip_prefix("oh-my-opencode") {
        if suffix.is_empty() || suffix.starts_with('@') {
            return format!("oh-my-openagent{suffix}");
        }
    }
    plugin_name.to_string()
}

pub fn get_opencode_dir() -> PathBuf {
    if let Some(override_dir) = get_opencode_override_dir() {
        return override_dir;
    }

    crate::config::get_home_dir()
        .join(".config")
        .join("opencode")
}

pub fn get_opencode_config_path() -> PathBuf {
    let dir = get_opencode_dir();
    let jsonc_path = dir.join("opencode.jsonc");
    let json_path = dir.join("opencode.json");

    if jsonc_path.exists() {
        jsonc_path
    } else {
        json_path
    }
}

/// 获取 OpenCode SQLite 数据库路径
/// 优先级: OPENCODE_DB 环境变量 > XDG_DATA_HOME > ~/.local/share/opencode
pub fn get_opencode_db_path() -> PathBuf {
    // 支持 OPENCODE_DB 环境变量覆盖（忽略空字符串）
    if let Ok(custom_path) = std::env::var("OPENCODE_DB") {
        if !custom_path.is_empty() {
            let path = PathBuf::from(&custom_path);
            if path.is_absolute() {
                return path;
            }
            // 相对路径基于数据目录
            return get_opencode_data_dir().join(path);
        }
    }

    get_opencode_data_dir().join("opencode.db")
}

fn get_opencode_data_dir() -> PathBuf {
    // 尊重 XDG_DATA_HOME（按 XDG 规范，空字符串视为未设置）
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        if !xdg_data.is_empty() {
            return PathBuf::from(xdg_data).join("opencode");
        }
    }

    // OpenCode 使用 xdg-basedir，不遵守 macOS/Windows 平台约定，
    // 所有平台默认都落在 ~/.local/share/opencode
    crate::config::get_home_dir()
        .join(".local")
        .join("share")
        .join("opencode")
}

#[allow(dead_code)]
pub fn get_opencode_env_path() -> PathBuf {
    get_opencode_dir().join(".env")
}

pub fn get_opencode_auth_path() -> PathBuf {
    crate::config::get_home_dir()
        .join(".local")
        .join("share")
        .join("opencode")
        .join("auth.json")
}

pub fn get_opencode_go_auth_key() -> Result<Option<String>, AppError> {
    let path = get_opencode_auth_path();
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    let auth: Value = serde_json::from_str(&content).map_err(|e| {
        AppError::Config(format!(
            "Failed to parse OpenCode auth: {}: {e}",
            path.display()
        ))
    })?;

    Ok(auth
        .get(OPENCODE_GO_PROVIDER_ID)
        .and_then(|value| value.as_object())
        .and_then(|entry| entry.get("key"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToString::to_string))
}

pub fn has_opencode_go_auth() -> Result<bool, AppError> {
    get_opencode_go_auth_key().map(|key| key.is_some())
}

// ---------------------------------------------------------------------------
// Raw file I/O (preserves original content for CST round-trip editing)
// ---------------------------------------------------------------------------

fn read_config_raw() -> Result<String, AppError> {
    let path = get_opencode_config_path();
    if !path.exists() {
        return Ok(String::from("{}"));
    }
    std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))
}

fn write_config_raw(content: &str) -> Result<(), AppError> {
    let path = get_opencode_config_path();
    write_text_file(&path, content)?;
    log::debug!("OpenCode config written to {path:?}");
    Ok(())
}

fn parse_cst(raw: &str) -> Result<CstRootNode, AppError> {
    CstRootNode::parse(raw, &ParseOptions::default())
        .map_err(|e| AppError::Message(format!("Failed to parse JSONC config: {e:?}")))
}

pub fn serde_value_to_cst(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(arr) => CstInputValue::Array(arr.iter().map(serde_value_to_cst).collect()),
        Value::Object(obj) => CstInputValue::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), serde_value_to_cst(v)))
                .collect(),
        ),
    }
}

/// Deep-merge a `serde_json::Value::Object` into an existing CST object.
///
/// For each key in `source`:
///   - If both the CST and source values are objects → recurse (preserves comments inside)
///   - Otherwise → shallow `set_value()` (replace leaf values)
///   - New keys → append
///
/// Keys present in CST but absent from `source` are left untouched (preserves
/// unknown fields like `google_auth`).
#[allow(dead_code)]
pub fn deep_merge_cst_object(
    cst_obj: &jsonc_parser::cst::CstObject,
    source: &serde_json::Map<String, Value>,
) {
    for (key, value) in source {
        match value {
            Value::Object(child_map) => {
                // Both sides are objects → recurse to preserve inner comments
                let nested = cst_obj.object_value_or_set(key);
                deep_merge_cst_object(&nested, child_map);
            }
            _ => {
                // Leaf value → shallow replace
                let cst_value = serde_value_to_cst(value);
                if let Some(existing) = cst_obj.get(key) {
                    existing.set_value(cst_value);
                } else {
                    cst_obj.append(key, cst_value);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CST helpers — set/remove properties while preserving comments & formatting
// ---------------------------------------------------------------------------

fn cst_set_object_property(section: &str, key: &str, value: &Value) -> Result<(), AppError> {
    let raw = read_config_raw()?;
    let root = parse_cst(&raw)?;
    let root_obj = root.object_value_or_set();
    let section_obj = root_obj.object_value_or_set(section);

    let cst_value = serde_value_to_cst(value);

    if let Some(existing) = section_obj.get(key) {
        existing.set_value(cst_value);
    } else {
        section_obj.append(key, cst_value);
    }

    write_config_raw(&root.to_string())
}

fn cst_remove_object_property(section: &str, key: &str) -> Result<(), AppError> {
    let raw = read_config_raw()?;
    let root = parse_cst(&raw)?;
    let root_obj = root.object_value_or_set();

    if let Some(section_obj) = root_obj.object_value(section) {
        if let Some(prop) = section_obj.get(key) {
            prop.remove();
        }
    }

    write_config_raw(&root.to_string())
}

// ---------------------------------------------------------------------------
// Read operations (parse into serde_json::Value — strips comments)
// ---------------------------------------------------------------------------

pub fn read_opencode_config() -> Result<Value, AppError> {
    let path = get_opencode_config_path();

    if !path.exists() {
        return Ok(json!({
            "$schema": "https://opencode.ai/config.json"
        }));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    json5::from_str(&content).map_err(|e| {
        AppError::Config(format!(
            "Failed to parse OpenCode config: {}: {e}",
            path.display()
        ))
    })
}

#[allow(dead_code)]
pub fn write_opencode_config(config: &Value) -> Result<(), AppError> {
    let path = get_opencode_config_path();
    write_json_file(&path, config)?;

    log::debug!("OpenCode config written to {path:?}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Provider operations (CST-based — preserves comments)
// ---------------------------------------------------------------------------

pub fn get_providers() -> Result<Map<String, Value>, AppError> {
    let config = read_opencode_config()?;
    Ok(config
        .get("provider")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default())
}

pub fn set_provider(id: &str, config: Value) -> Result<(), AppError> {
    cst_set_object_property("provider", id, &config)
}

pub fn remove_provider(id: &str) -> Result<(), AppError> {
    cst_remove_object_property("provider", id)
}

pub fn get_typed_providers() -> Result<IndexMap<String, OpenCodeProviderConfig>, AppError> {
    let providers = get_providers()?;
    let mut result = IndexMap::new();

    for (id, value) in providers {
        match serde_json::from_value::<OpenCodeProviderConfig>(value.clone()) {
            Ok(config) => {
                result.insert(id, config);
            }
            Err(e) => {
                log::warn!("Failed to parse provider '{id}': {e}");
            }
        }
    }

    Ok(result)
}

pub fn set_typed_provider(id: &str, config: &OpenCodeProviderConfig) -> Result<(), AppError> {
    let value = serde_json::to_value(config).map_err(|e| AppError::JsonSerialize { source: e })?;
    set_provider(id, value)
}

// ---------------------------------------------------------------------------
// MCP operations (CST-based — preserves comments)
// ---------------------------------------------------------------------------

pub fn get_mcp_servers() -> Result<Map<String, Value>, AppError> {
    let config = read_opencode_config()?;
    Ok(config
        .get("mcp")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default())
}

pub fn set_mcp_server(id: &str, config: Value) -> Result<(), AppError> {
    cst_set_object_property("mcp", id, &config)
}

pub fn remove_mcp_server(id: &str) -> Result<(), AppError> {
    cst_remove_object_property("mcp", id)
}

// ---------------------------------------------------------------------------
// Plugin operations (CST-based — preserves comments)
// ---------------------------------------------------------------------------

pub fn add_plugin(plugin_name: &str) -> Result<(), AppError> {
    let raw = read_config_raw()?;
    let root = parse_cst(&raw)?;
    let root_obj = root.object_value_or_set();
    let normalized_plugin_name = canonicalize_plugin_name(plugin_name);
    let plugins = root_obj.array_value_or_set("plugin");

    // Mutual exclusion: standard OMO and OMO Slim cannot coexist as plugins
    if matches_any_plugin_prefix(&normalized_plugin_name, &STANDARD_OMO_PLUGIN_PREFIXES)
        || matches_any_plugin_prefix(&normalized_plugin_name, &SLIM_OMO_PLUGIN_PREFIXES)
    {
        let to_remove: Vec<_> = plugins
            .elements()
            .into_iter()
            .filter(|el| {
                el.as_string_lit()
                    .and_then(|s| s.decoded_value().ok())
                    .map(|s| {
                        matches_any_plugin_prefix(&s, &STANDARD_OMO_PLUGIN_PREFIXES)
                            || matches_any_plugin_prefix(&s, &SLIM_OMO_PLUGIN_PREFIXES)
                    })
                    .unwrap_or(false)
            })
            .collect();
        for node in to_remove {
            node.remove();
        }
    }

    let already_exists = plugins.elements().iter().any(|el| {
        el.as_string_lit()
            .and_then(|s| s.decoded_value().ok())
            .map(|s| s == normalized_plugin_name)
            .unwrap_or(false)
    });

    if !already_exists {
        plugins.append(CstInputValue::String(normalized_plugin_name));
    }

    write_config_raw(&root.to_string())
}

pub fn remove_plugins_by_prefixes(prefixes: &[&str]) -> Result<(), AppError> {
    let raw = read_config_raw()?;
    let root = parse_cst(&raw)?;
    let root_obj = root.object_value_or_set();

    if let Some(plugins) = root_obj.array_value("plugin") {
        let to_remove: Vec<_> = plugins
            .elements()
            .into_iter()
            .filter(|el| {
                el.as_string_lit()
                    .and_then(|s| s.decoded_value().ok())
                    .map(|s| matches_any_plugin_prefix(&s, prefixes))
                    .unwrap_or(false)
            })
            .collect();
        for node in to_remove {
            node.remove();
        }

        if plugins.elements().is_empty() {
            if let Some(prop) = root_obj.get("plugin") {
                prop.remove();
            }
        }
    }

    write_config_raw(&root.to_string())
}
