use crate::cli::args::DepsArgs;
use crate::engine::deps;
use crate::error::PatchError;
use crate::output::CommandOutput;
use crate::output::{json, text};
use serde_json::{json, Map, Value};

pub fn run(args: &DepsArgs) -> Result<CommandOutput, PatchError> {
    let result = deps::run(&args.path, &args.scope)?;
    let mut output = CommandOutput::with_parts(
        "deps",
        text::deps::render(&result),
        json::deps::render(&result),
        result.diagnostics,
        true,
    );
    output.meta = meta_for_deps(&result.data);

    if let Some(budget) = args.budget {
        output.text = crate::budget::apply(&output.text, budget);
    }

    Ok(output)
}

fn meta_for_deps(data: &deps::DepsData) -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("path".into(), json!(data.path));
    meta.insert("local_uses".into(), json!(data.uses_local.len()));
    meta.insert("external_uses".into(), json!(data.uses_external.len()));
    meta.insert("dependents".into(), json!(data.used_by.len()));
    meta.insert("stability".into(), json!("medium"));
    meta.insert("noise".into(), json!("medium"));
    meta.insert("truncated".into(), json!(data.truncated_dependents > 0));
    meta
}
