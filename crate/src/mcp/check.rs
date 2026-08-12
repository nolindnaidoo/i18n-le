//! `check_catalogues` — the one tool this server offers.
//!
//! It takes catalogue *contents* directly and touches no filesystem. An
//! agent already has file-read tools; duplicating them here would add a
//! path-traversal surface for no capability.
//!
//! **The library is required, never detected.** Identification reads
//! manifests, config files, directory layouts and source call sites —
//! none of which exist on this surface. A tool that guessed instead
//! would be the thing this whole crate stopped doing, so the caller
//! names the library or gets a refusal.
//!
//! **Only keys, tokens and counts come back.** A translation catalogue
//! is a product's entire user-facing voice, and an agent asking whether
//! Spanish is complete does not need the Spanish. The privacy boundary
//! is the same one the CLI has, because it is the same code: findings
//! carry `Evidence`, and `Evidence` has no variant that holds a
//! sentence.

use serde_json::{Value, json};

use crate::library::{self, Id};
use crate::locale;
use crate::scan::{self, Document, ScanOptions, System};

const DEFAULT_MAX_RESULTS: usize = 500;
const MAX_MAX_RESULTS: usize = 5000;

pub(crate) fn definition() -> Value {
    json!({
        "name": "check_catalogues",
        "description": "Audit a set of translation catalogues against one of them and report what \
                        is structurally wrong: missing and extra keys, placeholders dropped or \
                        renamed in translation, constructs from another i18n convention, empty \
                        values, keys defined twice, and a path that is an object in one locale \
                        and a string in another. Takes file contents directly and reads no files. \
                        Only key names and structural facts are returned — never a translated \
                        string. The i18n library must be named: identifying it needs manifests, \
                        config files and source call sites, none of which this surface can see, \
                        and guessing the placeholder grammar from content is exactly what this \
                        tool does not do.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "library": {
                    "type": "string",
                    "enum": library::names(),
                    "description": "Which i18n library wrote these catalogues. It supplies the \
                                    placeholder grammar, the plural model and which keys are \
                                    metadata.",
                },
                "files": {
                    "type": "array",
                    "minItems": 1,
                    "description": "The catalogues to audit. JSON, nested or flat.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "File name, e.g. \"pt-BR.json\" or \
                                                \"bundle.l10n.pt-br.json\". Used to label findings \
                                                and, when locale is absent, to work out which \
                                                locale this is.",
                            },
                            "locale": {
                                "type": "string",
                                "description": "The language tag, e.g. \"pt-BR\". Omit for the \
                                                base catalogue, or omit on every file to have it \
                                                read from the names.",
                            },
                            "content": { "type": "string", "description": "The file contents." },
                        },
                        "required": ["path", "content"],
                        "additionalProperties": false,
                    },
                },
                "source": {
                    "type": "string",
                    "description": "Which catalogue is the contract — a path or a language tag. \
                                    Without it, exactly one English candidate must exist or the \
                                    audit is refused rather than guessed at.",
                },
                "keysAreSource": {
                    "type": "boolean",
                    "default": false,
                    "description": "The key is itself the English string, as in a VS Code \
                                    bundle.l10n.json.",
                },
                "maxResults": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_MAX_RESULTS,
                    "default": DEFAULT_MAX_RESULTS,
                    "description": format!(
                        "Cap on returned findings (default {DEFAULT_MAX_RESULTS}). \
                         meta.truncated reports whether any were dropped."
                    ),
                },
            },
            "required": ["library", "files"],
            "additionalProperties": false,
        },
    })
}

pub(crate) fn run(arguments: &Value) -> Result<Value, String> {
    let library = read_library(arguments)?;
    let documents = read_files(arguments)?;
    let max_results = read_max_results(arguments)?;
    let keys_are_source = arguments
        .get("keysAreSource")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let report = scan::report_for(
        &documents,
        System {
            library,
            version: None,
            layout: None,
            keys_are_source,
            evidence: Vec::new(),
        },
        &ScanOptions {
            source: arguments
                .get("source")
                .and_then(Value::as_str)
                .map(str::to_string),
            keys_are_source,
        },
    )?;

    // The cap matters less than the flag: a silently shortened answer is
    // wrong in the most expensive way, and this is a tool whose whole
    // job is telling you what is absent.
    let truncated = report.findings.len() > max_results;
    let diagnostics: Vec<Value> = report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "severity": diagnostic.severity,
                "code": diagnostic.code,
                "message": format!("{}: {}", diagnostic.file, diagnostic.message),
            })
        })
        .collect();

    let mut report = report;
    report.findings.truncate(max_results);
    let count = report.findings.len();
    let data = serde_json::to_value(&report).expect("a report serializes");

    Ok(super::envelope(
        "check_catalogues",
        &data,
        count,
        &diagnostics,
        truncated,
    ))
}

fn read_library(arguments: &Value) -> Result<Id, String> {
    let named = arguments
        .get("library")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "library is required and must be one of {}. It cannot be worked out from file \
                 contents alone, and guessing it is what this tool exists not to do.",
                library::names().join(", ")
            )
        })?;
    Id::parse(named).ok_or_else(|| {
        format!(
            "{named} is not a library this reads. Try one of {}.",
            library::names().join(", ")
        )
    })
}

/// Every locale settled before the audit starts.
///
/// A caller that knows the locales says so and the names are not read at
/// all. A caller that leaves any of them out has all of them worked out
/// from the names together — which is the only way the name of a
/// `bundle.l10n.pt-br.json` can be told from that of a
/// `package.nls.json`, and mixing a supplied answer into that would
/// change what the rest resolve to.
fn read_files(arguments: &Value) -> Result<Vec<Document>, String> {
    let invalid =
        "files is required and must be a non-empty array of { path, content }".to_string();
    let items = arguments
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid.clone())?;
    if items.is_empty() {
        return Err(invalid);
    }

    let mut names = Vec::new();
    let mut contents = Vec::new();
    let mut supplied = Vec::new();
    for item in items {
        let path = item
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid.clone())?;
        let content = item
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid.clone())?;
        names.push(path.to_string());
        contents.push(content.to_string());
        supplied.push(match item.get("locale").and_then(Value::as_str) {
            None => None,
            Some(tag) => Some(
                locale::canonicalise(tag).ok_or_else(|| format!("{tag} is not a language tag"))?,
            ),
        });
    }

    let locales = if supplied.iter().all(Option::is_some) {
        supplied
    } else {
        locale::locales_of(&names)?
    };

    Ok(names
        .into_iter()
        .zip(contents)
        .zip(locales)
        .map(|((name, content), locale)| Document {
            name,
            locale,
            content,
        })
        .collect())
}

/// Clamp quietly, reject loudly.
fn read_max_results(arguments: &Value) -> Result<usize, String> {
    let Some(raw) = arguments.get("maxResults") else {
        return Ok(DEFAULT_MAX_RESULTS);
    };
    let invalid = "maxResults must be a positive integer".to_string();
    let value = raw.as_u64().ok_or(invalid.clone())?;
    if value < 1 {
        return Err(invalid);
    }
    Ok(usize::try_from(value)
        .unwrap_or(MAX_MAX_RESULTS)
        .min(MAX_MAX_RESULTS))
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::corpus::document;

    const CASES: &str = include_str!("../../fixtures/mcp-check-catalogues.json");

    #[derive(Debug, Deserialize)]
    struct Case {
        name: String,
        files: Option<Vec<String>>,
        arguments: Value,
        expected: Option<Value>,
        #[serde(rename = "expectedError")]
        expected_error: Option<String>,
    }

    #[test]
    fn every_corpus_case_answers_identically() {
        let cases: Vec<Case> = serde_json::from_str(CASES).expect("the corpus is valid JSON");
        assert!(!cases.is_empty(), "the corpus is empty");

        for case in cases {
            let mut arguments = case.arguments.clone();
            if let Some(names) = &case.files {
                arguments["files"] = Value::Array(
                    names
                        .iter()
                        .map(|name| json!({ "path": name, "content": document(name) }))
                        .collect(),
                );
            }

            match (case.expected, case.expected_error) {
                (_, Some(expected)) => {
                    assert_eq!(
                        run(&arguments).expect_err(&case.name),
                        expected,
                        "{}",
                        case.name
                    );
                }
                (Some(expected), None) => {
                    assert_eq!(
                        run(&arguments).expect(&case.name),
                        expected,
                        "{}",
                        case.name
                    );
                }
                (None, None) => panic!("{} pins neither a result nor an error", case.name),
            }
        }
    }

    #[test]
    fn the_tool_name_is_pinned() {
        assert_eq!(definition()["name"], "check_catalogues");
    }

    /// **The honest split.** Nothing here can see a manifest, so nothing
    /// here may decide which library this is.
    #[test]
    fn the_library_is_required_and_says_why() {
        let error = run(&json!({ "files": [{ "path": "en.json", "content": "{}" }] }))
            .expect_err("a refusal");
        assert!(error.contains("library is required"), "{error}");
        assert!(error.contains("guessing"), "{error}");
        for known in library::names() {
            assert!(error.contains(known), "{error} omits {known}");
        }
    }

    #[test]
    fn a_library_this_does_not_read_is_refused() {
        let error = run(&json!({
            "library": "gettext",
            "files": [{ "path": "en.json", "content": "{}" }]
        }))
        .expect_err("a refusal");
        assert!(error.contains("gettext"), "{error}");
    }

    /// The property this crate rests on, asserted on the surface a model
    /// actually calls.
    #[test]
    fn no_translated_value_ever_reaches_the_answer() {
        let result = run(&json!({
            "library": "i18next",
            "files": [
                { "path": "en.json", "content": r#"{"a":"in {{timeframe}}","b":"Save"}"# },
                { "path": "es.json", "content": r#"{"a":"en {{periodo}}","z":"sobrante"}"# },
            ]
        }))
        .expect("a result");
        let rendered = serde_json::to_string(&result).expect("serializes");
        assert!(!rendered.contains("en {{periodo}}"), "{rendered}");
        assert!(!rendered.contains("sobrante\""), "{rendered}");
        assert!(rendered.contains("periodo"), "the token is the finding");
    }

    /// The schema may not offer a way to read values either.
    #[test]
    fn the_schema_offers_no_way_to_ask_for_values() {
        let definition = definition();
        let properties = definition["inputSchema"]["properties"]
            .as_object()
            .expect("properties");
        for absent in ["values", "showValues", "includeValues", "translate"] {
            assert!(!properties.contains_key(absent), "{absent} is offered");
        }
    }

    #[test]
    fn a_missing_files_argument_is_refused() {
        let error = run(&json!({ "library": "i18next" })).expect_err("a refusal");
        assert!(error.contains("files is required"), "{error}");
        assert!(run(&json!({ "library": "i18next", "files": [] })).is_err());
    }

    #[test]
    fn a_locale_that_is_not_a_language_tag_is_refused() {
        let error = run(&json!({
            "library": "i18next",
            "files": [{ "path": "en.json", "locale": "Spanish", "content": "{}" }]
        }))
        .expect_err("a refusal");
        assert!(error.contains("Spanish"), "{error}");
    }

    #[test]
    fn a_supplied_locale_is_used_instead_of_the_name() {
        let result = run(&json!({
            "library": "i18next",
            "files": [
                { "path": "one.json", "locale": "en", "content": r#"{"a":"one"}"# },
                { "path": "two.json", "locale": "es", "content": r#"{"a":"uno"}"# },
            ]
        }))
        .expect("a result");
        assert_eq!(result["data"]["source"]["path"], "one.json");
        assert_eq!(result["data"]["files"][1]["locale"], "es");
    }

    #[test]
    fn a_fractional_cap_is_refused() {
        let error = run(&json!({
            "library": "i18next",
            "files": [{ "path": "en.json", "content": "{}" }],
            "maxResults": 1.5
        }))
        .expect_err("a refusal");
        assert_eq!(error, "maxResults must be a positive integer");
    }

    /// The corpus feeds translations in as *arguments*; no expectation
    /// may carry one back out.
    #[test]
    fn no_corpus_expectation_carries_a_translated_value() {
        let cases: Vec<Value> = serde_json::from_str(CASES).expect("the corpus is valid JSON");
        for case in cases {
            let Some(expected) = case.get("expected") else {
                continue;
            };
            let rendered = serde_json::to_string(expected).expect("serializes");
            for translated in [
                "Bienvenido",
                "Bon retour",
                "Übersicht",
                "Kataloge",
                "Verificar",
                "Déconnexion",
                "Ola ",
            ] {
                assert!(!rendered.contains(translated), "{rendered}");
            }
        }
    }
}
