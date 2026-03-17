use crate::cli::args::SymbolFindArgs;
use crate::engine::symbol;
use crate::error::PatchError;
use crate::output::CommandOutput;
use crate::output::{json, text};
use serde_json::{json, Map, Value};

pub fn run(args: &SymbolFindArgs) -> Result<CommandOutput, PatchError> {
    let result = symbol::run(&args.query, &args.scope, args.kind, args.budget)?;
    let mut output = CommandOutput::with_parts(
        "symbol.find",
        text::symbol::render(&result),
        json::symbol::render(&result),
        result.diagnostics,
        true,
    );
    output.meta = meta_for_symbol_find(&result.data);

    Ok(output)
}

fn meta_for_symbol_find(data: &symbol::SymbolFindData) -> Map<String, Value> {
    let definitions = data
        .matches
        .iter()
        .filter(|item| item.kind == symbol::SymbolMatchKind::Definition)
        .count();
    let usages = data.matches.len().saturating_sub(definitions);

    let mut meta = Map::new();
    meta.insert("query".into(), json!(data.query));
    meta.insert("scope".into(), json!(data.scope));
    meta.insert("definitions".into(), json!(definitions));
    meta.insert("usages".into(), json!(usages));
    meta.insert("stability".into(), json!("medium"));
    meta.insert("noise".into(), json!("medium"));
    meta
}
