use crate::cli::args::MapArgs;
use crate::engine::map;
use crate::error::PatchError;
use crate::output::CommandOutput;
use crate::output::{json, text};
use serde_json::{json, Map, Value};

pub fn run(args: &MapArgs) -> Result<CommandOutput, PatchError> {
    let result = map::run(&args.scope, args.depth, args.budget)?;
    let meta = meta_for_map(&result);
    let mut output = CommandOutput::with_parts(
        "map",
        text::map::render(&result),
        json::map::render(&result),
        result.diagnostics,
        true,
    );
    output.meta = meta;

    Ok(output)
}

fn meta_for_map(result: &map::MapCommandResult) -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("scope".into(), json!(result.data.scope));
    meta.insert("depth".into(), json!(result.data.depth));
    meta.insert("total_files".into(), json!(result.data.total_files));
    meta.insert("total_tokens".into(), json!(result.data.total_tokens));
    meta.insert("stability".into(), json!("medium"));
    meta.insert("noise".into(), json!("medium"));
    meta.insert("truncated".into(), json!(result.truncated));
    meta
}
