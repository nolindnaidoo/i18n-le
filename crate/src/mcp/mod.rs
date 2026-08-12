//! The agent surface: the same audit over the Model Context Protocol on
//! stdio, so a model can ask whether a locale is complete rather than be
//! handed twenty-five catalogues and asked to diff them itself.
//!
//! Two rules the family's MCP surfaces established:
//!
//! - **An empty answer is not an error.** A set with nothing wrong comes
//!   back as an ordinary result carrying `ok: true` — the audit ran.
//!   Only a malformed question is a protocol error.
//! - **Refusals speak the caller's vocabulary.** An MCP caller has no
//!   command line, so no message here mentions a flag.
//!
//! Read-only by construction: nothing on this surface writes, and
//! nothing on it reaches a filesystem.

pub(crate) mod check;

use std::io::{BufRead, Write};
use std::process::ExitCode;

use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2025-06-18";

/// JSON-RPC error codes, from the spec.
const INVALID_PARAMS: i64 = -32602;
const METHOD_NOT_FOUND: i64 = -32601;

pub(crate) fn serve() -> ExitCode {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            return ExitCode::from(2);
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            // A frame that is not JSON has no id to answer against;
            // dropping it is the only honest option.
            continue;
        };
        let Some(response) = handle(&request) else {
            continue; // a notification: no reply
        };
        if writeln!(stdout, "{response}").is_err() || stdout.flush().is_err() {
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

fn handle(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method")?.as_str()?;
    // Notifications carry no id and get no reply.
    id.as_ref()?;

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "i18n-le", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => Ok(json!({ "tools": [check::definition()] })),
        "tools/call" => call_tool(request.get("params")),
        "ping" => Ok(json!({})),
        other => Err((
            METHOD_NOT_FOUND,
            format!("this server does not implement {other}"),
        )),
    };

    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }),
    })
}

/// Protocol failures (no tool named, an unknown tool) are JSON-RPC
/// errors; a tool that fails on its arguments returns a result carrying
/// `isError`, so a model reads the reason and reacts rather than
/// concluding the server is broken.
fn call_tool(params: Option<&Value>) -> Result<Value, (i64, String)> {
    let params = params.ok_or((INVALID_PARAMS, "no tool call was supplied".to_string()))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((INVALID_PARAMS, "the tool call named no tool".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    if name != "check_catalogues" {
        return Err((
            INVALID_PARAMS,
            format!("this server offers no tool named {name}"),
        ));
    }
    Ok(match check::run(&arguments) {
        Ok(result) => tool_result(&result),
        Err(message) => tool_failure(&message),
    })
}

/// The one result shape every tool returns: `{ ok, data, diagnostics,
/// meta }`, field for field the family's envelope.
///
/// **`ok` reports whether the audit ran, not whether the answer is
/// yes.** A locale full of missing keys is the answer, not a failure to
/// produce one — conflating the two would have a model report a broken
/// tool when what it actually learned is that the translations are
/// behind.
pub(crate) fn envelope(
    tool: &str,
    data: &Value,
    count: usize,
    diagnostics: &[Value],
    truncated: bool,
) -> Value {
    let ok = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic["severity"].as_str() == Some("error"));
    json!({
        "ok": ok,
        "data": data,
        "diagnostics": diagnostics,
        "meta": { "tool": tool, "count": count, "truncated": truncated },
    })
}

/// An MCP tool result: the envelope as text (what a model reads) and the
/// same envelope structured.
fn tool_result(envelope: &Value) -> Value {
    let text = serde_json::to_string_pretty(envelope).expect("an envelope serializes");
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": envelope,
        "isError": false,
    })
}

/// The tool could not run on the arguments given. `isError` so a model
/// reads the message and corrects itself.
fn tool_failure(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, params: &Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
    }

    fn call(name: &str, arguments: &Value) -> Value {
        handle(&request(
            "tools/call",
            &json!({ "name": name, "arguments": arguments }),
        ))
        .expect("a reply")
    }

    fn pair() -> Value {
        json!({ "library": "i18next", "files": [
            { "path": "en.json", "content": r#"{"a":"one","b":"two"}"# },
            { "path": "es.json", "content": r#"{"a":"uno"}"# },
        ]})
    }

    #[test]
    fn initialize_answers_with_the_protocol_version() {
        let response = handle(&request("initialize", &json!({}))).expect("a reply");
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(response["result"]["serverInfo"]["name"], "i18n-le");
    }

    #[test]
    fn tools_list_offers_the_tool() {
        let response = handle(&request("tools/list", &json!({}))).expect("a reply");
        let tools = response["result"]["tools"].as_array().expect("tools");
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names, ["check_catalogues"]);
    }

    #[test]
    fn a_notification_gets_no_reply() {
        let notification = json!({ "jsonrpc": "2.0", "method": "initialized" });
        assert!(handle(&notification).is_none());
    }

    #[test]
    fn an_unknown_method_is_a_protocol_error() {
        let response = handle(&request("does/not/exist", &json!({}))).expect("a reply");
        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn an_unknown_tool_is_a_protocol_error() {
        let response = call("envsync_le_check", &json!({}));
        assert_eq!(response["error"]["code"], INVALID_PARAMS);
    }

    /// A bad argument is the tool failing on what it was given, not the
    /// server breaking — so it comes back as a result carrying isError.
    #[test]
    fn a_missing_argument_is_a_tool_failure_not_a_protocol_error() {
        let response = call("check_catalogues", &json!({}));
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("a message")
                .contains("library is required")
        );
    }

    #[test]
    fn the_tool_reports_what_it_found() {
        let response = call("check_catalogues", &pair());
        let envelope = &response["result"]["structuredContent"];
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(envelope["meta"]["tool"], "check_catalogues");
        assert_eq!(envelope["data"]["status"], "findings");
        assert_eq!(envelope["data"]["findings"][0]["kind"], "missing-key");
        assert_eq!(envelope["data"]["findings"][0]["key"], "b");
    }

    /// Catalogues that agree are an ordinary result, not an empty one.
    #[test]
    fn agreeing_catalogues_are_an_ordinary_result() {
        let response = call(
            "check_catalogues",
            &json!({ "library": "i18next", "files": [
                { "path": "en.json", "content": r#"{"a":"one"}"# },
                { "path": "es.json", "content": r#"{"a":"uno"}"# },
            ]}),
        );
        let envelope = &response["result"]["structuredContent"];
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["data"]["status"], "clean");
    }

    /// The tool reaches no filesystem — the property that lets an agent
    /// call it anywhere, and it must not regress.
    #[test]
    fn the_tool_needs_no_filesystem() {
        let response = call(
            "check_catalogues",
            &json!({ "library": "i18next", "files": [
                { "path": "/definitely/not/here/en.json", "content": r#"{"a":"one"}"# },
            ]}),
        );
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(
            response["result"]["structuredContent"]["data"]["status"],
            "clean"
        );
    }

    /// Refusals speak the caller's vocabulary: an MCP caller has no
    /// command line, so no message may name a flag.
    #[test]
    fn no_message_mentions_a_command_line_flag() {
        let definitions = serde_json::to_string(&check::definition()).expect("serializes");
        assert!(!definitions.contains("--"), "{definitions}");

        for arguments in [
            json!({}),
            json!({ "library": "i18next", "files": [] }),
            json!({ "library": "i18next",
                    "files": [{ "path": "package.json", "content": "{}" },
                              { "path": "tsconfig.json", "content": "{}" }] }),
            json!({ "library": "i18next",
                    "files": [{ "path": "es.json", "content": "{}" },
                              { "path": "fr.json", "content": "{}" }] }),
            json!({ "library": "i18next",
                    "files": [{ "path": "en.json", "content": "{}" }], "source": "de" }),
        ] {
            let rendered =
                serde_json::to_string(&call("check_catalogues", &arguments)).expect("serializes");
            assert!(!rendered.contains("--"), "{rendered}");
        }
    }

    #[test]
    fn the_tool_returns_the_family_envelope() {
        let envelope = &call("check_catalogues", &pair())["result"]["structuredContent"];
        assert!(envelope["ok"].is_boolean(), "{envelope}");
        assert!(!envelope["data"].is_null(), "{envelope}");
        assert!(envelope["diagnostics"].is_array(), "{envelope}");
        assert!(envelope["meta"]["tool"].is_string(), "{envelope}");
        assert!(envelope["meta"]["count"].is_number(), "{envelope}");
        assert!(envelope["meta"]["truncated"].is_boolean(), "{envelope}");
    }

    #[test]
    fn a_capped_answer_says_that_it_was_capped() {
        let response = call(
            "check_catalogues",
            &json!({
                "library": "i18next",
                "files": [
                    { "path": "en.json", "content": r#"{"a":"1","b":"2","c":"3"}"# },
                    { "path": "es.json", "content": "{}" },
                ],
                "maxResults": 1,
            }),
        );
        let envelope = &response["result"]["structuredContent"];
        assert_eq!(envelope["meta"]["truncated"], true);
        assert_eq!(envelope["meta"]["count"], 1);
    }
}
