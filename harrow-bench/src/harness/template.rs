use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use minijinja::{Environment, UndefinedBehavior};
use serde_json::Value;

pub fn render_template_file(
    path: &Path,
    context: &BTreeMap<String, Value>,
) -> Result<String, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("failed to read template {}: {e}", path.display()))?;
    render_template_str(&source, context)
        .map_err(|e| format!("failed to render template {}: {e}", path.display()))
}

pub fn render_template_str(
    source: &str,
    context: &BTreeMap<String, Value>,
) -> Result<String, String> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.add_template("inline", source)
        .map_err(|e| format!("failed to load template: {e}"))?;
    env.get_template("inline")
        .map_err(|e| format!("failed to fetch template: {e}"))?
        .render(context)
        .map_err(|e| format!("failed to render template: {e}"))
}
