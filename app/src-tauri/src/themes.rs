use std::path::Path;

use serde_json::Value;

// [THM-01]
const BUILTIN: &[(&str, &str)] = &[
    ("black-lab", include_str!("../../../themes/black-lab.toml")),
    ("dark", include_str!("../../../themes/dark.toml")),
    ("light", include_str!("../../../themes/light.toml")),
];

// [THM-02]
fn parse(id: &str, src: &str, builtin: bool) -> Option<Value> {
    let mut v: Value = toml::from_str(src).ok()?;
    let obj = v.as_object_mut()?;
    obj.insert("id".into(), Value::String(id.into()));
    obj.insert("builtin".into(), Value::Bool(builtin));
    Some(v)
}

// [THM-03]
pub fn list(user_dir: Option<&Path>) -> Vec<Value> {
    let mut out: Vec<Value> = BUILTIN
        .iter()
        .filter_map(|(id, src)| parse(id, src, true))
        .collect();
    if let Some(dir) = user_dir {
        if let Ok(read) = std::fs::read_dir(dir) {
            let mut files: Vec<_> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
            files.sort();
            for f in files {
                if f.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
                }
                let Some(stem) = f.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                    continue;
                };
                let Ok(src) = std::fs::read_to_string(&f) else {
                    continue;
                };
                if let Some(v) = parse(&format!("user:{stem}"), &src, false) {
                    out.push(v);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_themes_parse() {
        let themes = list(None);
        assert_eq!(themes.len(), 3);
        for t in &themes {
            assert!(t["colors"]["accent"].is_string());
            assert!(t["meters"]["clip"].is_string());
        }
        assert_eq!(themes[0]["colors"]["accent"], "#FF1654");
    }
}
