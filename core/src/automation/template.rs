//! `{{name}}` placeholders in snippets and playbook steps.
//!
//! - `{{name}}` asks for a value when the snippet runs; `{{name=default}}`
//!   pre-fills it.
//! - `{{host}}`, `{{hostname}}` and `{{user}}` are filled per host and never
//!   asked for.
//! - Values are inserted shell-quoted unless they only contain characters a
//!   shell treats literally, so a value can't run a second command. Write
//!   `rm {{path}}`, not `rm "{{path}}"`.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;

/// Filled per host, never asked for.
pub const BUILTINS: [&str; 3] = ["host", "hostname", "user"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    /// From `{{name=default}}`, if any.
    pub default: Option<String>,
}

/// One `{{...}}`: byte range, name and default.
struct Placeholder<'a> {
    start: usize,
    end: usize,
    name: &'a str,
    default: Option<&'a str>,
}

fn valid_name(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && s.len() <= 40
}

/// Finds placeholders; text that only looks like one (`{{ x y }}`, `{{}}`)
/// is left alone as literal text.
fn scan(text: &str) -> Vec<Placeholder<'_>> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = text[i..].find("{{") {
        let start = i + off;
        let Some(len) = text[start + 2..].find("}}") else { break };
        let inner = &text[start + 2..start + 2 + len];
        let end = start + 2 + len + 2;
        let (name, default) = match inner.split_once('=') {
            Some((n, d)) => (n.trim(), Some(d)),
            None => (inner.trim(), None),
        };
        if valid_name(name) && !inner.contains('\n') {
            out.push(Placeholder { start, end, name, default });
            i = end;
        } else {
            i = start + 2;
        }
    }
    out
}

/// Variables to ask for, in order of first use (built-ins left out). The
/// first default given for a name wins.
pub fn variables<'a>(texts: impl IntoIterator<Item = &'a str>) -> Vec<Variable> {
    let mut out: Vec<Variable> = Vec::new();
    for t in texts {
        for p in scan(t) {
            if BUILTINS.contains(&p.name) {
                continue;
            }
            match out.iter_mut().find(|v| v.name == p.name) {
                Some(v) => {
                    if v.default.is_none() {
                        v.default = p.default.map(str::to_owned);
                    }
                }
                None => out.push(Variable { name: p.name.to_owned(), default: p.default.map(str::to_owned) }),
            }
        }
    }
    out
}

/// Quotes `v` only when a shell would treat any of it specially.
pub fn quote_value(v: &str) -> String {
    let plain = !v.is_empty()
        && v.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@' | '%' | '+' | ',' | '=')
        });
    if plain {
        v.to_owned()
    } else {
        shell_quote(v)
    }
}

/// Per-host values for the built-in placeholders.
pub fn builtins(host: &str, hostname: &str, user: &str) -> HashMap<String, String> {
    HashMap::from([
        ("host".to_owned(), host.to_owned()),
        ("hostname".to_owned(), hostname.to_owned()),
        ("user".to_owned(), user.to_owned()),
    ])
}

/// Replaces placeholders. `quote` shell-quotes values (commands); paths and
/// names given to SFTP or as arguments are inserted as they are. Fails
/// listing every variable without a value or default.
pub fn render(text: &str, values: &HashMap<String, String>, quote: bool) -> Result<String, Vec<String>> {
    let mut out = String::with_capacity(text.len());
    let mut missing: BTreeMap<String, ()> = BTreeMap::new();
    let mut last = 0;
    for p in scan(text) {
        out.push_str(&text[last..p.start]);
        let v = values.get(p.name).map(String::as_str).filter(|v| !v.is_empty()).or(p.default);
        match v {
            Some(v) => out.push_str(&if quote { quote_value(v) } else { v.to_owned() }),
            None => {
                missing.insert(p.name.to_owned(), ());
            }
        }
        last = p.end;
    }
    out.push_str(&text[last..]);
    if missing.is_empty() {
        Ok(out)
    } else {
        Err(missing.into_keys().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vals(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn finds_variables() {
        let v =
            variables(["docker logs --tail {{lines=200}} {{container}}", "echo {{container}} {{host}} {{ not one }}"]);
        assert_eq!(
            v,
            vec![
                Variable { name: "lines".into(), default: Some("200".into()) },
                Variable { name: "container".into(), default: None },
            ]
        );
        assert!(variables(["awk '{print $1}' {{}} {{1x}}"]).is_empty());
    }

    #[test]
    fn renders_and_quotes() {
        let t = "docker logs --tail {{lines=200}} {{container}} # {{host}}";
        let mut v = vals(&[("container", "web-1")]);
        v.extend(builtins("server01", "10.0.0.1", "admin"));
        assert_eq!(render(t, &v, true).unwrap(), "docker logs --tail 200 web-1 # server01");
        let v = vals(&[("container", "x; rm -rf /"), ("lines", "5")]);
        assert_eq!(render("docker logs {{container}}", &v, true).unwrap(), "docker logs 'x; rm -rf /'");
        assert_eq!(render("cat {{p}}", &vals(&[("p", "it's")]), true).unwrap(), r"cat 'it'\''s'");
        assert_eq!(render("{{p}}", &vals(&[("p", "a b")]), false).unwrap(), "a b");
        assert_eq!(render("{{a}} {{b}} {{a}}", &HashMap::new(), true).unwrap_err(), vec!["a", "b"]);
        // An empty value falls back to the default.
        assert_eq!(render("{{n=3}}", &vals(&[("n", "")]), true).unwrap(), "3");
        assert_eq!(render("awk '{{print}}'", &HashMap::new(), true).unwrap_err(), vec!["print"]);
        assert_eq!(render("no vars {{ x y }}", &HashMap::new(), true).unwrap(), "no vars {{ x y }}");
    }
}
