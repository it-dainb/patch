use crate::cli::args::SearchTextArgs;
use crate::engine::search;
use crate::error::DrailError;
use crate::output::json::envelope::NextItem;
use crate::output::CommandOutput;
use crate::output::{json, text};
use serde_json::{json, Map, Value};

pub fn run(args: &SearchTextArgs) -> Result<CommandOutput, DrailError> {
    let result = search::run_text(&args.query, &args.scope, args.budget)?;
    let next = next_for_search(&result);
    let diagnostics = diagnostics_without_suggestions(&result.diagnostics);
    let mut output = CommandOutput::with_parts(
        "search.text",
        text::search::render(&result),
        json::search::render(&result),
        diagnostics,
        true,
    );
    output.meta = meta_for_search(&result.data);
    output.next = next;

    Ok(output)
}

fn diagnostics_without_suggestions(
    diagnostics: &[crate::output::json::envelope::Diagnostic],
) -> Vec<crate::output::json::envelope::Diagnostic> {
    diagnostics
        .iter()
        .cloned()
        .map(|mut diagnostic| {
            diagnostic.suggestion = None;
            diagnostic
        })
        .collect()
}

fn next_for_search(result: &search::SearchCommandResult) -> Vec<NextItem> {
    if result.data.matches.is_empty() && looks_like_slash_delimited_regex(&result.data.query) {
        return vec![crate::output::suggestion(
            "Retry this search using regex mode for slash-delimited input",
            format!(
                "drail search regex {:?} --scope {}",
                &result.data.query[1..result.data.query.len() - 1],
                result.data.scope
            ),
        )];
    }

    Vec::new()
}

fn looks_like_slash_delimited_regex(query: &str) -> bool {
    query.starts_with('/') && query.ends_with('/') && query.len() > 2
}

fn meta_for_search(data: &search::SearchData) -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("query".into(), json!(data.query));
    meta.insert("scope".into(), json!(data.scope));
    meta.insert("matches".into(), json!(data.matches.len()));
    meta.insert("mode".into(), json!(data.mode));
    meta.insert("stability".into(), json!("high"));
    meta.insert("noise".into(), json!("medium"));
    meta
}
