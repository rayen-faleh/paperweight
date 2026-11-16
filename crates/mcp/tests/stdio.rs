//! Phase 2 gate: start the real `paperweight-mcp` binary, connect with rmcp's
//! client over its stdio, call every tool (and the error paths), read the
//! resource and the prompt, and assert stdout carried nothing but JSON-RPC.
//!
//! Set `PAPERWEIGHT_TRANSCRIPT=path.md` to save every request and response.

use std::process::Stdio;
use std::sync::{Arc, Mutex};

use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, GetPromptRequestParams, ReadResourceRequestParams,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type Log = Arc<Mutex<Vec<(char, String)>>>;

fn obj(v: Value) -> serde_json::Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => panic!("arguments must be an object"),
    }
}

struct Harness {
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
}

impl Harness {
    async fn call(&self, name: &str, args: Value) -> CallToolResult {
        let mut p = CallToolRequestParams::new(name.to_string());
        if !args.is_null() {
            p = p.with_arguments(obj(args));
        }
        self.client
            .call_tool(p)
            .await
            .unwrap_or_else(|e| panic!("{name}: protocol error {e}"))
    }

    async fn ok(&self, name: &str, args: Value) -> Value {
        let r = self.call(name, args).await;
        assert_ne!(r.is_error, Some(true), "{name} failed: {:?}", text(&r));
        // A short text summary plus the JSON.
        assert!(r.content.len() >= 2, "{name}: summary and JSON");
        r.structured_content.clone().expect("structured content")
    }

    async fn err(&self, name: &str, args: Value) -> String {
        let r = self.call(name, args).await;
        assert_eq!(r.is_error, Some(true), "{name} should fail: {:?}", text(&r));
        text(&r)
    }
}

fn text(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test(flavor = "multi_thread")]
async fn every_tool_over_real_stdio() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("mcp.db");
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_paperweight-mcp"))
        .env("PAPERWEIGHT_DB", &db)
        .env("PAPERWEIGHT_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn paperweight-mcp");

    let log: Log = Arc::new(Mutex::new(vec![]));

    // server stdout -> (record) -> client
    let (client_read, mut tee_out) = tokio::io::duplex(1 << 20);
    let child_out = child.stdout.take().unwrap();
    let l = log.clone();
    let out_task = tokio::spawn(async move {
        let mut lines = BufReader::new(child_out).lines();
        let mut all = vec![];
        while let Ok(Some(line)) = lines.next_line().await {
            l.lock().unwrap().push(('<', line.clone()));
            all.push(line.clone());
            if tee_out
                .write_all(format!("{line}\n").as_bytes())
                .await
                .is_err()
            {
                break;
            }
        }
        all
    });
    // client -> (record) -> server stdin
    let (mut tee_in, client_write) = tokio::io::duplex(1 << 20);
    let mut child_in = child.stdin.take().unwrap();
    let l = log.clone();
    let in_task = tokio::spawn(async move {
        let mut lines = BufReader::new(&mut tee_in).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            l.lock().unwrap().push(('>', line.clone()));
            if child_in
                .write_all(format!("{line}\n").as_bytes())
                .await
                .is_err()
            {
                break;
            }
        }
        drop(child_in);
    });
    let mut child_err = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = tokio::io::AsyncReadExt::read_to_string(&mut child_err, &mut s).await;
        s
    });

    let client = ().serve((client_read, client_write)).await.expect("initialize");
    let info = client.peer_info().expect("server info").clone();
    assert_eq!(info.server_info.as_ref().unwrap().name, "paperweight");
    let instructions = info.instructions.clone().unwrap_or_default();
    assert!(instructions.contains("get_brief"), "{instructions}");
    assert!(instructions.contains("confirm"), "{instructions}");
    let h = Harness { client };

    // ---- tools/list ----
    let tools = h.client.list_all_tools().await.unwrap();
    let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "add_item",
            "complete_item",
            "delete_item",
            "get_brief",
            "list_items",
            "plan_week",
            "snooze_item",
            "update_item"
        ]
    );
    for t in &tools {
        let d = t.description.as_deref().unwrap_or("");
        assert!(d.len() > 150, "{} needs a real description", t.name);
    }
    let add = tools.iter().find(|t| t.name == "add_item").unwrap();
    let schema = serde_json::to_string(&add.input_schema).unwrap();
    assert!(
        schema.contains("requirements") && schema.contains("steps"),
        "{schema}"
    );
    assert!(
        add.description
            .as_deref()
            .unwrap()
            .contains("next <weekday>")
    );

    // ---- add_item ----
    let added = h
        .ok(
            "add_item",
            json!({
                "title": "Residence card renewal",
                "due": "in 9 days",
                "kind": "paperwork",
                "notes": "Immigration office, morning",
                "estimated_minutes": 120,
                "steps": [{"title": "Get photo", "soft_due": "in 2 days"}, {"title": "Fill in form"}],
                "requirements": ["Passport", "Current card", "Photo 4x3"]
            }),
        )
        .await;
    let item = &added["item"];
    let id = item["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("pw-") && id.len() == 7);
    assert_eq!(item["kind"], "paperwork");
    assert_eq!(item["all_day"], true);
    assert_eq!(item["steps"].as_array().unwrap().len(), 2);
    assert!(item["steps"][0]["soft_due_at"].is_string());
    assert_eq!(item["requirements"].as_array().unwrap().len(), 3);
    assert!(item["due_label"].is_string() && item["due_day"].is_string());

    let rent = h
        .ok(
            "add_item",
            json!({"title": "Rent", "due": "2026-01-31", "kind": "payment", "recurrence": {"freq": "monthly"}, "estimated_minutes": 15}),
        )
        .await;
    let rent_id = rent["item"]["id"].as_str().unwrap().to_string();
    // The first rent is in the past, so the server says so.
    let r = h
        .call("add_item", json!({"title": "Late", "due": "2020-01-01"}))
        .await;
    assert!(text(&r).contains("already in the past"), "{}", text(&r));

    let big = h
        .ok("add_item", json!({"title": "Thesis chapter", "due": "in 3 days", "kind": "assignment", "estimated_minutes": 600}))
        .await;
    let big_id = big["item"]["id"].as_str().unwrap().to_string();

    // add_item errors
    let e = h
        .err(
            "add_item",
            json!({"title": "Form", "due": "end of the month"}),
        )
        .await;
    assert!(e.contains("ISO 8601") && e.contains("It is now"), "{e}");
    let e = h
        .err("add_item", json!({"title": "   ", "due": "tomorrow"}))
        .await;
    assert!(e.contains("title"), "{e}");
    let e = h.err("add_item", json!({"title": "x", "due": "tomorrow", "recurrence": {"freq": "weekly", "interval": 0}})).await;
    assert!(e.contains("interval"), "{e}");

    // Wrong argument types are rejected without killing the server.
    let bad = h
        .client
        .call_tool(CallToolRequestParams::new("add_item").with_arguments(obj(json!({"title": 5}))))
        .await;
    match bad {
        Ok(r) => assert_eq!(r.is_error, Some(true)),
        Err(e) => assert!(
            e.to_string().to_lowercase().contains("invalid") || e.to_string().contains("-32602"),
            "{e}"
        ),
    }
    let unknown = h
        .client
        .call_tool(CallToolRequestParams::new("no_such_tool"))
        .await;
    assert!(unknown.is_err() || unknown.unwrap().is_error == Some(true));

    // ---- update_item ----
    let step0 = item["steps"][0]["id"].as_i64().unwrap();
    let req0 = item["requirements"][0]["id"].as_i64().unwrap();
    let req2 = item["requirements"][2]["id"].as_i64().unwrap();
    let up = h
        .ok(
            "update_item",
            json!({
                "id": id,
                "due": "in 10 days 10:00",
                "complete_steps": [step0],
                "add_steps": [{"title": "Book appointment"}],
                "check_requirements": [req0],
                "remove_requirements": [req2],
                "add_requirements": ["Revenue stamp"],
                "source": "https://example.org/form.pdf"
            }),
        )
        .await;
    let u = &up["item"];
    assert_eq!(u["all_day"], false);
    assert_eq!(u["steps"][0]["done"], true);
    assert_eq!(u["steps"].as_array().unwrap().len(), 3);
    assert_eq!(u["requirements"][0]["done"], true);
    let reqs: Vec<&str> = u["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["text"].as_str().unwrap())
        .collect();
    assert_eq!(reqs, ["Passport", "Current card", "Revenue stamp"]);
    assert_eq!(u["source"], "https://example.org/form.pdf");
    let up = h
        .ok(
            "update_item",
            json!({"id": id, "source": "", "reopen_steps": [step0]}),
        )
        .await;
    assert!(up["item"]["source"].is_null());
    assert_eq!(up["item"]["steps"][0]["done"], false);

    let e = h
        .err("update_item", json!({"id": "pw-xxxx", "title": "y"}))
        .await;
    assert_eq!(e, "No item with id pw-xxxx. Call list_items to find it.");
    let e = h
        .err("update_item", json!({"id": id, "complete_steps": [99999]}))
        .await;
    assert!(
        e.contains("no step with id 99999") && e.contains("Get photo"),
        "{e}"
    );
    let e = h
        .err("update_item", json!({"id": id, "due": "someday"}))
        .await;
    assert!(e.contains("ISO 8601"), "{e}");
    let e = h
        .err("update_item", json!({"id": id, "status": "archived"}))
        .await;
    assert!(e.contains("delete_item"), "{e}");

    // ---- snooze_item ----
    let sn = h
        .ok("snooze_item", json!({"id": id, "until": "tomorrow 15:00"}))
        .await;
    assert!(sn["item"]["snoozed_until"].is_string());
    assert_eq!(
        sn["item"]["due_at"], up["item"]["due_at"],
        "snooze does not move the deadline"
    );
    assert_eq!(sn["item"]["snoozed"], true);
    let e = h
        .err(
            "snooze_item",
            json!({"id": id, "until": "2020-01-01T10:00"}),
        )
        .await;
    assert!(e.contains("past"), "{e}");
    let e = h
        .err("snooze_item", json!({"id": "pw-zzzz", "until": "tomorrow"}))
        .await;
    assert!(e.contains("list_items"), "{e}");
    let cleared = h
        .ok("update_item", json!({"id": id, "clear_snooze": true}))
        .await;
    assert!(cleared["item"]["snoozed_until"].is_null());

    // ---- complete_item ----
    let done = h.ok("complete_item", json!({"id": rent_id})).await;
    assert_eq!(done["item"]["status"], "done");
    let next = &done["next"];
    assert_eq!(next["status"], "open");
    assert_eq!(next["recurrence"]["freq"], "monthly");
    assert_ne!(next["id"], rent_id);
    let e = h.err("complete_item", json!({"id": rent_id})).await;
    assert!(e.contains("already done"), "{e}");
    let e = h.err("complete_item", json!({"id": "pw-nope"})).await;
    assert!(e.contains("No item with id pw-nope"), "{e}");

    // ---- list_items ----
    let all = h.ok("list_items", json!({})).await;
    assert!(all["count"].as_u64().unwrap() >= 3);
    let found = h.ok("list_items", json!({"query": "revenue STAMP"})).await;
    assert_eq!(found["count"], 1);
    assert_eq!(found["items"][0]["id"], id);
    let pay = h
        .ok("list_items", json!({"kind": "payment", "status": "all"}))
        .await;
    assert_eq!(pay["count"], 2, "done rent and next rent");
    let ranged = h
        .ok(
            "list_items",
            json!({"due_from": "tomorrow", "due_to": "in 5 days"}),
        )
        .await;
    assert!(
        ranged["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["id"] == big_id)
    );
    assert!(
        !ranged["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["id"] == id)
    );
    let e = h.err("list_items", json!({"due_from": "last week"})).await;
    assert!(e.contains("due_from"), "{e}");

    // ---- get_brief ----
    let b = h.ok("get_brief", Value::Null).await;
    assert!(b["now_local"].is_string() && b["today"].is_string());
    assert!(
        b["overdue"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["title"] == "Late")
    );
    assert!(
        b["next_7_days"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["id"] == big_id)
    );

    // ---- plan_week ----
    let plan = h.ok("plan_week", Value::Null).await;
    let at_risk = plan["at_risk"].as_array().unwrap();
    assert!(
        at_risk
            .iter()
            .any(|r| r["item_id"] == big_id && r["reason"].as_str().unwrap().contains("600")),
        "the 600-minute item cannot fit in a 4-hour block: {plan}"
    );
    assert!(plan["sessions"].is_array());

    // ---- delete_item ----
    let del = h.ok("delete_item", json!({"id": big_id})).await;
    assert_eq!(del["item"]["status"], "archived");
    let e = h
        .err("update_item", json!({"id": big_id, "title": "z"}))
        .await;
    assert!(e.contains("archive"), "{e}");
    let e = h.err("delete_item", json!({"id": big_id})).await;
    assert!(e.contains("archive"), "{e}");
    let after = h.ok("list_items", json!({"status": "all"})).await;
    assert!(
        !after["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["id"] == big_id),
        "deleted items are hidden"
    );

    // ---- resource ----
    let resources = h.client.list_all_resources().await.unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "paperweight://routine");
    let r = h
        .client
        .read_resource(ReadResourceRequestParams::new("paperweight://routine"))
        .await
        .unwrap();
    let body = match &r.contents[0] {
        rmcp::model::ResourceContents::TextResourceContents { text, .. } => text.clone(),
        other => panic!("expected text, got {other:?}"),
    };
    let routine: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(routine["routine"].as_array().unwrap().len(), 28);
    assert_eq!(routine["settings"]["day_rollover"], "02:00");
    assert!(
        h.client
            .read_resource(ReadResourceRequestParams::new("paperweight://nope"))
            .await
            .is_err()
    );

    // ---- prompt ----
    let prompts = h.client.list_all_prompts().await.unwrap();
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "weekly_review");
    let p = h
        .client
        .get_prompt(GetPromptRequestParams::new("weekly_review"))
        .await
        .unwrap();
    let ptext = match &p.messages[0].content {
        rmcp::model::ContentBlock::Text(t) => t.text.clone(),
        other => panic!("{other:?}"),
    };
    assert!(ptext.contains("get_brief") && ptext.contains("plan_week"));

    // Parallel tool calls (assistants often send several at once) must all
    // succeed, not fail with "database is locked".
    let (r1, r2, r3, r4, r5, r6) = tokio::join!(
        h.call(
            "add_item",
            json!({"title": "Parallel 1", "due": "tomorrow 11:00", "requirements": ["a"]})
        ),
        h.call(
            "add_item",
            json!({"title": "Parallel 2", "due": "tomorrow 12:15", "steps": [{"title": "s"}]})
        ),
        h.call(
            "add_item",
            json!({"title": "Parallel 3", "due": "in 2 days"})
        ),
        h.call(
            "update_item",
            json!({"id": id, "add_requirements": ["extra req"]})
        ),
        h.call(
            "add_item",
            json!({"title": "Parallel 4", "due": "in 3 days"})
        ),
        h.call("snooze_item", json!({"id": id, "until": "tomorrow 09:00"})),
    );
    for r in [r1, r2, r3, r4, r5, r6] {
        assert_ne!(r.is_error, Some(true), "parallel call failed: {}", text(&r));
    }
    let par = h.ok("list_items", json!({"query": "Parallel"})).await;
    assert_eq!(par["count"], 4);

    // The server is still alive after every error above.
    h.ok("get_brief", Value::Null).await;

    // ---- shut down and check stdout ----
    h.client.cancel().await.unwrap();
    let status = tokio::time::timeout(std::time::Duration::from_secs(10), child.wait())
        .await
        .expect("server exits when stdin closes")
        .unwrap();
    assert!(status.success(), "clean exit: {status}");
    let _ = in_task.await;
    let stdout_lines = out_task.await.unwrap();
    let stderr = err_task.await.unwrap();

    assert!(!stdout_lines.is_empty());
    for line in &stdout_lines {
        let v: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("non-JSON on stdout ({e}): {line}"));
        assert_eq!(v["jsonrpc"], "2.0", "not JSON-RPC: {line}");
        assert!(
            v.get("id").is_some() || v.get("method").is_some(),
            "not a JSON-RPC message: {line}"
        );
    }
    assert!(
        stderr.contains("paperweight-mcp starting"),
        "logs go to stderr: {stderr}"
    );

    if let Ok(path) = std::env::var("PAPERWEIGHT_TRANSCRIPT") {
        write_transcript(&path, &log.lock().unwrap());
    }
}

/// Render the captured exchange as Markdown: each request with its response.
fn write_transcript(path: &str, log: &[(char, String)]) {
    let mut responses = std::collections::HashMap::new();
    for (dir, line) in log {
        if *dir == '<'
            && let Ok(v) = serde_json::from_str::<Value>(line)
            && let Some(id) = v.get("id")
        {
            responses.insert(id.to_string(), v);
        }
    }
    let mut out = String::from(
        "# MCP transcript\n\nEvery request the test client sent to the real `paperweight-mcp` binary over stdio, with the server's response, \
         captured byte-for-byte from the pipes by `crates/mcp/tests/stdio.rs` (run with `PAPERWEIGHT_TRANSCRIPT=docs/mcp-transcript.md`). \
         Long responses are shown in full.\n\n",
    );
    let mut n = 0;
    for (dir, line) in log {
        if *dir != '>' {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let method = req["method"].as_str().unwrap_or("");
        let title = if method == "tools/call" {
            format!(
                "tools/call → {}",
                req["params"]["name"].as_str().unwrap_or("?")
            )
        } else {
            method.to_string()
        };
        n += 1;
        out.push_str(&format!(
            "## {n}. {title}\n\nRequest:\n\n```json\n{}\n```\n\n",
            serde_json::to_string_pretty(&req).unwrap()
        ));
        if let Some(id) = req.get("id") {
            if let Some(resp) = responses.get(&id.to_string()) {
                out.push_str(&format!(
                    "Response:\n\n```json\n{}\n```\n\n",
                    serde_json::to_string_pretty(resp).unwrap()
                ));
            }
        } else {
            out.push_str("(notification, no response)\n\n");
        }
    }
    std::fs::write(path, out).expect("write transcript");
}
