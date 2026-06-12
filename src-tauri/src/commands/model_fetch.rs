//! 模型列表获取命令
//!
//! 提供 Tauri 命令，供前端在供应商表单中获取可用模型列表。

use crate::services::model_fetch::{self, FetchedModel};
use std::process::Command;

/// 获取供应商的可用模型列表
///
/// 使用 OpenAI 兼容的 GET /v1/models 端点。优先使用 `models_url` 精确覆写；
/// 否则对 baseURL 生成候选列表（含「剥离 Anthropic 兼容子路径」兜底），按序尝试。
#[tauri::command(rename_all = "camelCase")]
pub async fn fetch_models_for_config(
    base_url: String,
    api_key: String,
    is_full_url: Option<bool>,
    models_url: Option<String>,
    custom_user_agent: Option<String>,
) -> Result<Vec<FetchedModel>, String> {
    // 与转发 / 检测路径共用 parse_custom_user_agent：非法 UA 静默忽略（不阻断取模型）。
    let user_agent = crate::provider::parse_custom_user_agent(custom_user_agent.as_deref())
        .ok()
        .flatten();
    model_fetch::fetch_models(
        &base_url,
        &api_key,
        is_full_url.unwrap_or(false),
        models_url.as_deref(),
        user_agent,
    )
    .await
}

#[tauri::command]
pub fn fetch_opencode_go_models() -> Result<Vec<FetchedModel>, String> {
    if crate::opencode_config::get_opencode_go_auth_key()
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("OpenCode Go auth is missing".to_string());
    }

    let output = Command::new("opencode")
        .args(["models", crate::opencode_config::OPENCODE_GO_PROVIDER_ID])
        .output()
        .map_err(|e| format!("Failed to run opencode models: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("opencode models exited with {}", output.status)
        } else {
            stderr
        });
    }

    Ok(parse_opencode_models_output(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn parse_opencode_models_output(output: &str) -> Vec<FetchedModel> {
    let prefix = format!("{}/", crate::opencode_config::OPENCODE_GO_PROVIDER_ID);
    let mut models: Vec<FetchedModel> = output
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| FetchedModel {
            id: id.to_string(),
            owned_by: Some(crate::opencode_config::OPENCODE_GO_PROVIDER_ID.to_string()),
        })
        .collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);
    models
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_opencode_models_output_keeps_only_opencode_go_ids() {
        let models = parse_opencode_models_output(
            r#"
[OpencodeSkillful][warn] ignored log line
opencode-go/kimi-k2.6
opencode-go/deepseek-v4-flash
anthropic/claude-sonnet-4-6
opencode-go/kimi-k2.6
"#,
        );

        let ids: Vec<_> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, vec!["deepseek-v4-flash", "kimi-k2.6"]);
        assert!(models.iter().all(|model| {
            model.owned_by.as_deref() == Some(crate::opencode_config::OPENCODE_GO_PROVIDER_ID)
        }));
    }
}
