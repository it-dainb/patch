use crate::cli::args::FilesArgs;
use crate::engine::files;
use crate::error::DrailError;
use crate::output::json::envelope::NextItem;
use crate::output::CommandOutput;
use crate::output::{json, text};
use serde_json::{json, Map, Value};

pub fn run(args: &FilesArgs) -> Result<CommandOutput, DrailError> {
    let result = files::run(&args.pattern, &args.scope, args.budget)?;
    let next = next_for_files(&result);
    let diagnostics = diagnostics_without_suggestions(&result.diagnostics);
    let mut output = CommandOutput::with_parts(
        "files",
        text::files::render(&result),
        json::files::render(&result),
        diagnostics,
        true,
    );
    output.meta = meta_for_files(&result.data);
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

fn next_for_files(result: &files::FilesCommandResult) -> Vec<NextItem> {
    if !result.data.files.is_empty() {
        return Vec::new();
    }

    let command = result
        .diagnostics
        .iter()
        .find_map(|diagnostic| diagnostic.suggestion.as_deref())
        .map(str::to_owned)
        .or_else(|| {
            infer_available_extension(&result.data.scope).map(|extension| {
                format!(
                    "drail files \"*.{extension}\" --scope {}",
                    result.data.scope
                )
            })
        });

    command
        .into_iter()
        .map(|command| {
            crate::output::suggestion(
                format!(
                    "Try a broader or available file pattern for {}",
                    result.data.scope
                ),
                command.trim_start_matches("Try: "),
            )
        })
        .collect()
}

fn infer_available_extension(scope: &str) -> Option<String> {
    let entries = std::fs::read_dir(scope).ok()?;
    let mut extensions = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.path().extension()?.to_str().map(str::to_owned))
        .collect::<Vec<_>>();
    extensions.sort();
    extensions.dedup();
    extensions.into_iter().next()
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
