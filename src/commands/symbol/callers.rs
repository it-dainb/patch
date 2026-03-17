use crate::cli::args::SymbolCallersArgs;
use crate::engine::symbol;
use crate::error::PatchError;
use crate::output::CommandOutput;
use crate::output::{json, text};
use serde_json::{json, Map, Value};

pub fn run(args: &SymbolCallersArgs) -> Result<CommandOutput, PatchError> {
    let result = symbol::run_callers(&args.query, &args.scope, args.budget)?;
    let meta = meta_for_symbol_callers(&result);
    let mut output = CommandOutput::with_parts(
        "symbol.callers",
        text::symbol::render_callers(&result),
        json::symbol::render_callers(&result),
        result.diagnostics,
        true,
    );
    output.meta = meta;

    Ok(output)
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
