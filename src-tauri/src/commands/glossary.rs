//! AI glossary-suggestion Tauri commands — Phase 3.4 (mode 3).
//!
//! Propose-and-approve, like Smart Suggest:
//!   - `glossary_suggest_estimate` — pure cost/scope preview, no network.
//!   - `glossary_suggest` — async. Returns candidate terms; adds nothing.
//!     The renderer merges the accepted ones into `project.glossary`.
//!
//! (The post-pass `op_apply_glossary` that corrects captions from an existing
//! glossary lives in `commands::operations`.)

use crate::commands::polish::PolishEstimate;
use crate::error::AppResult;
use crate::model::Project;
use crate::services::llm::glossary_suggest::{
    self, build_glossary_system_prompt, build_glossary_user_prompt, estimate_output_tokens,
    SuggestedTerm,
};
use crate::services::llm::{self, estimate_cost_usd, rough_token_count, ClaudeModel, LlmConfig};

#[tauri::command]
pub fn glossary_suggest_estimate(
    project: Project,
    model: ClaudeModel,
) -> AppResult<PolishEstimate> {
    let system = build_glossary_system_prompt(&project.language);
    let user = build_glossary_user_prompt(&project);
    let input = rough_token_count(&system) + rough_token_count(&user);
    let output = estimate_output_tokens(&project);

    let caption_count = project
        .captions
        .iter()
        .filter(|c| !c.words.is_empty())
        .count();
    Ok(PolishEstimate {
        caption_count,
        estimated_input_tokens: input,
        estimated_output_tokens: output,
        estimated_cost_usd: estimate_cost_usd(model, input, output),
        model_id: model.id().to_string(),
    })
}

/// Scan the transcript for likely-misrecognized terms. Returns candidates for
/// review; nothing is added to the project here.
#[tauri::command]
pub async fn glossary_suggest(
    project: Project,
    model: ClaudeModel,
    api_key: Option<String>,
) -> AppResult<Vec<SuggestedTerm>> {
    let has_captions = project.captions.iter().any(|c| !c.words.is_empty());
    if !has_captions {
        return Ok(vec![]);
    }

    let system = build_glossary_system_prompt(&project.language);
    let user = build_glossary_user_prompt(&project);
    let max_tokens = estimate_output_tokens(&project).clamp(256, 4096) as u32;

    let key = crate::services::secrets::resolve(
        api_key,
        crate::services::secrets::SecretProvider::Anthropic,
        "ANTHROPIC_API_KEY",
    );
    let config = LlmConfig {
        model,
        api_key: key,
    };

    let response = llm::complete(&config, &system, &user, max_tokens).await?;
    glossary_suggest::parse_glossary_suggestions(&response)
}
