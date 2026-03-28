use crate::cli::args::SymbolCallersArgs;
use crate::engine::symbol;
use crate::error::DrailError;
use crate::output::json::envelope::NextItem;
use crate::output::CommandOutput;
use crate::output::{json, text};
use serde_json::{json, Map, Value};

pub fn run(args: &SymbolCallersArgs) -> Result<CommandOutput, DrailError> {
    let result = symbol::run_callers(&args.query, &args.scope, args.budget)?;
    let next = next_for_symbol_callers(&result);
    let meta = meta_for_symbol_callers(&result);
    let mut output = CommandOutput::with_parts(
        "symbol.callers",
        text::symbol::render_callers(&result),
        json::symbol::render_callers(&result),
        result.diagnostics,
        true,
    );
    output.meta = meta;
    output.next = next;

    Ok(output)
}

fn next_for_symbol_callers(result: &symbol::SymbolCallersCommandResult) -> Vec<NextItem> {
    if result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "callers_relation_not_meaningful")
    {
        return vec![crate::output::suggestion(
            "Inspect symbol definitions directly when callers are not meaningful",
            format!(
                "drail symbol find {:?} --scope {}",
                result.data.query, result.data.scope
            ),
        )];
    }

    if result.data.callers.is_empty() && result.data.impact.is_empty() {
        return vec![crate::output::suggestion(
            "Fallback to symbol or text search when callers are unavailable",
            format!(
                "drail symbol find {:?} --scope {}",
                result.data.query, result.data.scope
            ),
        )];
    }

    Vec::new()
}

fn meta_for_symbol_callers(result: &symbol::SymbolCallersCommandResult) -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("query".into(), json!(result.data.query));
    meta.insert("scope".into(), json!(result.data.scope));
    meta.insert("direct_call_sites".into(), json!(result.data.callers.len()));
    meta.insert("second_hop_sites".into(), json!(result.data.impact.len()));
    meta.insert("stability".into(), json!("medium"));
    meta.insert("noise".into(), json!("high"));
    meta.insert("truncated".into(), json!(result.truncated));
    meta
}
