use crate::cli::args::FilesArgs;
use crate::engine::files;
use crate::error::PatchError;
use crate::output::CommandOutput;
use crate::output::{json, text};
use serde_json::{json, Map, Value};

pub fn run(args: &FilesArgs) -> Result<CommandOutput, PatchError> {
    let result = files::run(&args.pattern, &args.scope, args.budget)?;
    let mut output = CommandOutput::with_parts(
        "files",
        text::files::render(&result),
        json::files::render(&result),
        result.diagnostics,
        true,
    );
    output.meta = meta_for_files(&result.data);

    Ok(output)
}

fn meta_for_files(data: &files::FilesData) -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("pattern".into(), json!(data.pattern));
    meta.insert("scope".into(), json!(data.scope));
    meta.insert("files".into(), json!(data.files.len()));
    meta.insert("stability".into(), json!("high"));
    meta.insert("noise".into(), json!("low"));
    meta
}
