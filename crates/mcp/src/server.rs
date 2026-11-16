//! Tools, the routine resource and the weekly_review prompt.
//!
//! The server holds no state between calls: every call opens the database,
//! does its work and closes it.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use paperweight_core::brief::{ItemView, brief};
use paperweight_core::dates::{Due, parse_due};
use paperweight_core::model::{Freq, ItemPatch, Kind, NewItem, NewStep, Recurrence, Status};
use paperweight_core::planner::plan_week;
use paperweight_core::{Clock, Error as CoreError, ItemFilter, Store};
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, GetPromptResult, Implementation, ListResourcesResult,
    PaginatedRequestParams, PromptMessage, ReadResourceRequestParams, ReadResourceResponse,
    ReadResourceResult, Resource, ResourceContents, Role, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, prompt, prompt_handler, prompt_router, tool,
    tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

pub const ROUTINE_URI: &str = "paperweight://routine";

pub const INSTRUCTIONS: &str = "Paperweight is the user's local deadline tracker for paperwork, assignments, payments and life admin. \
It reminds them on its own, so your job is to keep it accurate with as little effort from them as possible: when they mention something with a deadline \
(a form, a bill, an assignment, an appointment), add it with add_item in one go, including any steps and the checklist of what is needed. \
When they ask what is coming up, what to do today, or how their week looks, call get_brief first. Use plan_week to suggest when to work on things. \
After adding an item, always confirm it back to the user in one short sentence: the title, the due date and time in their words, and the id. \
Their day ends at the day_rollover time (usually 02:00), not midnight, so 01:00 on Wednesday counts as Tuesday night. Item ids look like pw-7k3f.";

macro_rules! due_rules {
    () => {
        "Dates: send ISO 8601 when you can — \"2026-11-20\" means all day (due by the end of that day), \"2026-11-20T17:00\" is a time in the user's timezone, \
         and an offset or Z (\"2026-11-20T08:00Z\") is honoured. Also accepted, and nothing else: today, tonight, tomorrow, a weekday name \
         (the next such day, today included), next <weekday> (the next such day after today), in N days, in N weeks — each optionally followed \
         by a time like 17:00, 5pm or 5:30pm. The user's day ends at 02:00 (the day rollover), so \"today\" at 00:30 is still the previous day, \
         and a time like 01:00 belongs to the night of the given day. Anything else is rejected with an error that states the current local date and time; \
         then convert to ISO 8601 yourself and retry. Never guess a date the user did not give — ask them."
    };
}

#[derive(Debug, Clone)]
pub struct Paperweight {
    db_path: PathBuf,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

// ---------- parameter types ----------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StepInput {
    /// What to do, e.g. "Get a passport photo taken".
    pub title: String,
    /// Optional soft target for this step, same date format as `due`.
    #[serde(default)]
    pub soft_due: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecurrenceInput {
    /// "none", "daily", "weekly", "monthly" or "yearly". Monthly items on the
    /// 29th–31st land on the last day of shorter months.
    pub freq: Freq,
    /// Every N days/weeks/months/years. Default 1.
    #[serde(default)]
    pub interval: Option<u32>,
}

impl RecurrenceInput {
    fn to_core(&self) -> Recurrence {
        Recurrence {
            freq: self.freq,
            interval: self.interval.unwrap_or(1),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddItemParams {
    /// Short name the user would recognise, e.g. "Renew residence card". Put details in `notes`.
    pub title: String,
    /// When it is due. See the date rules in the tool description.
    pub due: String,
    /// paperwork, assignment, payment, appointment, social or other. Default other.
    #[serde(default)]
    pub kind: Option<Kind>,
    /// Free-text details: where to go, who to contact, amounts, context.
    #[serde(default)]
    pub notes: Option<String>,
    /// Where it came from: a URL or a local file path (e.g. the PDF of the form).
    #[serde(default)]
    pub source: Option<String>,
    /// Minutes of work it needs, used by plan_week. Default 30. Use 0 for things that need no preparation (a dinner).
    #[serde(default)]
    pub estimated_minutes: Option<u32>,
    /// Set if it repeats, e.g. {"freq": "monthly"} for rent.
    #[serde(default)]
    pub recurrence: Option<RecurrenceInput>,
    /// Ordered sub-tasks, if the user mentioned any or they are obvious.
    #[serde(default)]
    pub steps: Option<Vec<StepInput>>,
    /// The "what's needed" checklist: documents to bring, forms to fill, where to submit. One line each.
    #[serde(default)]
    pub requirements: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateItemParams {
    /// The item id, e.g. "pw-7k3f".
    pub id: String,
    /// New title.
    #[serde(default)]
    pub title: Option<String>,
    /// New deadline, same format as add_item's `due`. Moving the deadline resets its reminders.
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub kind: Option<Kind>,
    /// Replaces the notes entirely. To append, send the old notes plus the new text.
    #[serde(default)]
    pub notes: Option<String>,
    /// New source URL or path; an empty string removes it.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub estimated_minutes: Option<u32>,
    /// New repeat rule; {"freq": "none"} stops it repeating.
    #[serde(default)]
    pub recurrence: Option<RecurrenceInput>,
    /// "open" to reopen a done item. To finish an item use complete_item; to delete use delete_item.
    #[serde(default)]
    pub status: Option<Status>,
    /// True removes an active snooze so reminders resume.
    #[serde(default)]
    pub clear_snooze: Option<bool>,
    /// Steps to append, in order.
    #[serde(default)]
    pub add_steps: Option<Vec<StepInput>>,
    /// Step ids (numbers from the item's `steps`) to tick off.
    #[serde(default)]
    pub complete_steps: Option<Vec<i64>>,
    /// Step ids to untick.
    #[serde(default)]
    pub reopen_steps: Option<Vec<i64>>,
    /// Step ids to delete.
    #[serde(default)]
    pub remove_steps: Option<Vec<i64>>,
    /// Requirement lines to append to the "what's needed" checklist.
    #[serde(default)]
    pub add_requirements: Option<Vec<String>>,
    /// Requirement ids (numbers from the item's `requirements`) to tick off.
    #[serde(default)]
    pub check_requirements: Option<Vec<i64>>,
    /// Requirement ids to untick.
    #[serde(default)]
    pub uncheck_requirements: Option<Vec<i64>>,
    /// Requirement ids to delete.
    #[serde(default)]
    pub remove_requirements: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IdParams {
    /// The item id, e.g. "pw-7k3f". Call list_items if you do not know it.
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SnoozeParams {
    /// The item id, e.g. "pw-7k3f".
    pub id: String,
    /// When reminders may resume. Same format as add_item's `due`; a date without a time means the start of that day.
    pub until: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StatusFilter {
    Open,
    Done,
    /// Open and done (deleted items are never listed).
    All,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListItemsParams {
    /// "open" (default), "done", or "all" (open and done). Deleted items are never listed.
    #[serde(default)]
    pub status: Option<StatusFilter>,
    /// Only this kind.
    #[serde(default)]
    pub kind: Option<Kind>,
    /// Only items due on or after this date (same format as add_item's `due`; a bare date means from the start of that day).
    #[serde(default)]
    pub due_from: Option<String>,
    /// Only items due on or before this date (a bare date includes that whole day).
    #[serde(default)]
    pub due_to: Option<String>,
    /// Case-insensitive text to find in titles, notes, sources, steps and requirements.
    #[serde(default)]
    pub query: Option<String>,
    /// Maximum number of items (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WeeklyReviewArgs {
    /// Optional focus for the review, e.g. "paperwork only".
    #[serde(default)]
    pub focus: Option<String>,
}

// ---------- helpers ----------

type ToolResult = Result<CallToolResult, McpError>;

/// A successful result: a short summary for the model plus structured JSON.
fn ok(summary: String, data: Value) -> ToolResult {
    let pretty = serde_json::to_string_pretty(&data).unwrap_or_default();
    let mut r = CallToolResult::structured(data);
    r.content = vec![ContentBlock::text(summary), ContentBlock::text(pretty)];
    Ok(r)
}

/// A tool error the model can act on. Never a protocol error, never a crash.
fn fail(msg: impl Into<String>) -> ToolResult {
    Ok(CallToolResult::error(vec![ContentBlock::text(msg.into())]))
}

fn view(
    store: &Store,
    item: paperweight_core::Item,
    now: DateTime<Utc>,
) -> Result<ItemView, CoreError> {
    Ok(ItemView::new(item, &store.clock()?, now))
}

fn describe(v: &ItemView) -> String {
    let mut s = format!(
        "{} \"{}\" — due {}{}, {}",
        v.item.id,
        v.item.title,
        v.due_label,
        if v.item.all_day { " (all day)" } else { "" },
        v.item.kind
    );
    if !v.item.requirements.is_empty() {
        s.push_str(&format!(
            "; needs {}/{} ready",
            v.requirements_done,
            v.item.requirements.len()
        ));
    }
    if !v.item.steps.is_empty() {
        s.push_str(&format!(
            "; steps {}/{} done",
            v.steps_done,
            v.item.steps.len()
        ));
    }
    if !v.item.recurrence.is_none() {
        s.push_str(&format!(
            "; repeats {}{}",
            v.item.recurrence.freq,
            if v.item.recurrence.interval > 1 {
                format!(" every {}", v.item.recurrence.interval)
            } else {
                String::new()
            }
        ));
    }
    if v.snoozed {
        s.push_str(&format!(
            "; reminders snoozed until {}",
            v.item
                .snoozed_until
                .map(|t| t.to_rfc3339())
                .unwrap_or_default()
        ));
    }
    s
}

fn parse(field: &str, text: &str, now: DateTime<Utc>, clock: &Clock) -> Result<Due, String> {
    parse_due(text, now, clock).map_err(|e| format!("{field}: {e}"))
}

fn steps_in(
    steps: &[StepInput],
    now: DateTime<Utc>,
    clock: &Clock,
) -> Result<Vec<NewStep>, String> {
    steps
        .iter()
        .map(|s| {
            Ok(NewStep {
                title: s.title.clone(),
                soft_due_at: s
                    .soft_due
                    .as_deref()
                    .map(|d| parse("steps.soft_due", d, now, clock).map(|d| d.at))
                    .transpose()?,
            })
        })
        .collect()
}

impl Paperweight {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }

    /// Run `f` with a fresh database connection on a blocking thread.
    async fn with_store<T, F>(&self, f: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&mut Store, DateTime<Utc>) -> Result<T, String> + Send + 'static,
    {
        let path = self.db_path.clone();
        tokio::task::spawn_blocking(move || {
            let mut store = Store::open(&path).map_err(|e| {
                format!(
                    "Could not open the Paperweight database at {}: {e}",
                    path.display()
                )
            })?;
            f(&mut store, Utc::now())
        })
        .await
        .map_err(|e| format!("internal error: {e}"))?
    }
}

fn s(e: CoreError) -> String {
    e.to_string()
}

#[tool_router]
impl Paperweight {
    #[tool(
        description = concat!(
            "Create a new tracked item (a deadline). Use this whenever the user mentions something they must do by a date: a form to hand in, \
             a bill, an assignment, an appointment, plans with friends. Put everything you know in this one call — steps, the \"what's needed\" \
             checklist (requirements), notes, the source URL or file, a repeat rule — so the user never has to say it twice. \
             Do not use this to change an existing item (use update_item) or to record that something is finished (use complete_item). \
             If the user did not say when it is due, ask them before calling. \
             Returns the full item; afterwards confirm the title, due date and id back to the user. ",
            due_rules!()
        ),
        annotations(title = "Add item", read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = false)
    )]
    async fn add_item(&self, Parameters(p): Parameters<AddItemParams>) -> ToolResult {
        let res = self
            .with_store(move |store, now| {
                let clock = store.clock().map_err(s)?;
                let due = parse("due", &p.due, now, &clock)?;
                let mut new = NewItem::new(p.title, due.at, due.all_day);
                new.kind = p.kind.unwrap_or(Kind::Other);
                new.notes = p.notes.unwrap_or_default();
                new.source = p.source;
                new.estimated_minutes = p.estimated_minutes;
                new.recurrence = p
                    .recurrence
                    .as_ref()
                    .map(|r| r.to_core())
                    .unwrap_or_default();
                new.steps = steps_in(p.steps.as_deref().unwrap_or(&[]), now, &clock)?;
                new.requirements = p.requirements.unwrap_or_default();
                let item = store.add_item(new, now).map_err(s)?;
                view(store, item, now).map_err(s)
            })
            .await;
        match res {
            Ok(v) => {
                let warn = if v.overdue {
                    " Note: this deadline is already in the past, so it shows as overdue — check the date with the user."
                } else {
                    ""
                };
                ok(
                    format!(
                        "Added {}.{warn} Confirm this back to the user.",
                        describe(&v)
                    ),
                    json!({ "item": v }),
                )
            }
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = concat!(
            "Change one existing item: any of its fields, and its steps and requirements (add, tick, untick or remove them by their numeric ids, \
             which appear in the item's `steps` and `requirements`). Only the fields you send change. Use this when the user says a date moved, \
             adds detail, or ticks off part of the work. Do not use this to finish the whole item (use complete_item), to silence reminders \
             (use snooze_item) or to delete it (use delete_item). If you do not know the id, call list_items with a query first. \
             Returns the full item after the change. ",
            due_rules!()
        ),
        annotations(title = "Update item", read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn update_item(&self, Parameters(p): Parameters<UpdateItemParams>) -> ToolResult {
        let res = self
            .with_store(move |store, now| {
                let clock = store.clock().map_err(s)?;
                let mut patch = ItemPatch {
                    title: p.title,
                    kind: p.kind,
                    notes: p.notes,
                    estimated_minutes: p.estimated_minutes,
                    recurrence: p.recurrence.as_ref().map(|r| r.to_core()),
                    status: p.status,
                    ..Default::default()
                };
                if let Some(d) = p.due.as_deref() {
                    let due = parse("due", d, now, &clock)?;
                    patch.due = Some((due.at, due.all_day));
                }
                if let Some(src) = p.source {
                    patch.source = Some(Some(src).filter(|s| !s.trim().is_empty()));
                }
                if p.clear_snooze == Some(true) {
                    patch.snoozed_until = Some(None);
                }
                patch.add_steps = steps_in(p.add_steps.as_deref().unwrap_or(&[]), now, &clock)?;
                patch.set_steps_done = p
                    .complete_steps
                    .unwrap_or_default()
                    .into_iter()
                    .map(|i| (i, true))
                    .chain(
                        p.reopen_steps
                            .unwrap_or_default()
                            .into_iter()
                            .map(|i| (i, false)),
                    )
                    .collect();
                patch.remove_steps = p.remove_steps.unwrap_or_default();
                patch.add_requirements = p.add_requirements.unwrap_or_default();
                patch.set_requirements_done = p
                    .check_requirements
                    .unwrap_or_default()
                    .into_iter()
                    .map(|i| (i, true))
                    .chain(
                        p.uncheck_requirements
                            .unwrap_or_default()
                            .into_iter()
                            .map(|i| (i, false)),
                    )
                    .collect();
                patch.remove_requirements = p.remove_requirements.unwrap_or_default();
                let item = store.update_item(&p.id, patch, now).map_err(s)?;
                view(store, item, now).map_err(s)
            })
            .await;
        match res {
            Ok(v) => ok(format!("Updated {}.", describe(&v)), json!({ "item": v })),
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Mark an item as done. Use this when the user says they finished, submitted, paid or attended it. \
                       If the item repeats (e.g. monthly rent), this also creates the next occurrence and returns it as `next`; \
                       tell the user the next due date. Do not use this for ticking a single step (use update_item with complete_steps). \
                       To undo, call update_item with status \"open\".",
        annotations(
            title = "Complete item",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn complete_item(&self, Parameters(p): Parameters<IdParams>) -> ToolResult {
        let res = self
            .with_store(move |store, now| {
                let (done, next) = store.complete_item(&p.id, now).map_err(s)?;
                let done = view(store, done, now).map_err(s)?;
                let next = next.map(|n| view(store, n, now)).transpose().map_err(s)?;
                Ok((done, next))
            })
            .await;
        match res {
            Ok((done, next)) => {
                let mut summary =
                    format!("Marked {} \"{}\" as done.", done.item.id, done.item.title);
                if let Some(n) = &next {
                    summary.push_str(&format!(" It repeats; the next one is {}.", describe(n)));
                }
                ok(summary, json!({ "item": done, "next": next }))
            }
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Silence an item's reminders until a given time. The deadline does not move. Use this when the user says \
                       \"remind me later\", \"not now\" or \"I'll deal with it on Friday\". If the deadline itself changed, use update_item \
                       with `due` instead. `until` takes the same formats as add_item's `due` (ISO 8601, or today, tonight, tomorrow, a weekday, \
                       next <weekday>, in N days, in N weeks, with an optional time); a date without a time means the start of that day. \
                       To resume reminders early, call update_item with clear_snooze true.",
        annotations(
            title = "Snooze item",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn snooze_item(&self, Parameters(p): Parameters<SnoozeParams>) -> ToolResult {
        let res = self
            .with_store(move |store, now| {
                let clock = store.clock().map_err(s)?;
                let due = parse("until", &p.until, now, &clock)?;
                let until = if due.all_day {
                    clock.day_start(clock.deadline_day(due.at))
                } else {
                    due.at
                };
                let item = store.snooze_item(&p.id, until, now).map_err(s)?;
                view(store, item, now).map_err(s)
            })
            .await;
        match res {
            Ok(v) => ok(
                format!(
                    "Snoozed reminders for {} \"{}\" until {}. The deadline is unchanged: {}.",
                    v.item.id,
                    v.item.title,
                    v.item
                        .snoozed_until
                        .map(|t| t.to_rfc3339())
                        .unwrap_or_default(),
                    v.due_label
                ),
                json!({ "item": v }),
            ),
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Delete an item the user no longer needs to track (cancelled, added by mistake, a duplicate). It moves to the archive, \
                       where the user can restore it from the Paperweight app for 30 days. Do not use this when the item was finished — \
                       use complete_item so it counts as done. Confirm with the user before deleting unless they asked plainly.",
        annotations(
            title = "Delete item",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_item(&self, Parameters(p): Parameters<IdParams>) -> ToolResult {
        let res = self
            .with_store(move |store, now| {
                let item = store.delete_item(&p.id, now).map_err(s)?;
                view(store, item, now).map_err(s)
            })
            .await;
        match res {
            Ok(v) => ok(
                format!(
                    "Deleted {} \"{}\". It is in the archive and can be restored from the Paperweight app for 30 days.",
                    v.item.id, v.item.title
                ),
                json!({ "item": v }),
            ),
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Find items by status, kind, due-date range and text. Use this to look up an item's id before updating, completing, \
                       snoozing or deleting it, or to answer questions like \"what paperwork do I have?\" or \"what did I finish this month?\". \
                       For \"what's coming up\" or \"what should I do today\", use get_brief instead. Dates in due_from/due_to take the same formats \
                       as add_item's `due`. Results are sorted by deadline.",
        annotations(
            title = "List items",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_items(&self, Parameters(p): Parameters<ListItemsParams>) -> ToolResult {
        let res = self
            .with_store(move |store, now| {
                let clock = store.clock().map_err(s)?;
                let statuses = match p.status {
                    None | Some(StatusFilter::Open) => vec![Status::Open],
                    Some(StatusFilter::Done) => vec![Status::Done],
                    Some(StatusFilter::All) => vec![Status::Open, Status::Done],
                };
                let due_from = p
                    .due_from
                    .as_deref()
                    .map(|d| {
                        parse("due_from", d, now, &clock).map(|d| {
                            if d.all_day {
                                clock.day_start(clock.deadline_day(d.at))
                            } else {
                                d.at
                            }
                        })
                    })
                    .transpose()?;
                let due_to = p
                    .due_to
                    .as_deref()
                    .map(|d| parse("due_to", d, now, &clock).map(|d| d.at))
                    .transpose()?;
                let limit = p.limit.unwrap_or(50).clamp(1, 500);
                let items = store
                    .list_items(&ItemFilter {
                        statuses,
                        kinds: p.kind.into_iter().collect(),
                        due_from,
                        due_to,
                        text: p.query,
                        limit: Some(limit),
                    })
                    .map_err(s)?;
                Ok(items
                    .into_iter()
                    .map(|i| ItemView::new(i, &clock, now))
                    .collect::<Vec<_>>())
            })
            .await;
        match res {
            Ok(items) => {
                let summary = if items.is_empty() {
                    "No items match.".to_string()
                } else {
                    format!(
                        "{} item{}:\n{}",
                        items.len(),
                        if items.len() == 1 { "" } else { "s" },
                        items
                            .iter()
                            .map(|v| format!("- {}", v.one_line()))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                };
                ok(summary, json!({ "count": items.len(), "items": items }))
            }
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "What needs attention: overdue items, items due today, and items due in the next 7 days, each in priority order \
                       (earliest deadline first), plus the current local date and time. Call this first whenever the user asks what is \
                       coming up, what to do today or tonight, or how their week looks, and before suggesting what to work on. \
                       \"Today\" follows the user's day rollover (02:00 by default), so late-night deadlines count as today. Takes no parameters.",
        annotations(
            title = "Get brief",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_brief(&self) -> ToolResult {
        let res = self
            .with_store(|store, now| {
                let clock = store.clock().map_err(s)?;
                let items = store.open_items().map_err(s)?;
                Ok(brief(items, &clock, now))
            })
            .await;
        match res {
            Ok(b) => {
                let section = |name: &str, v: &[ItemView]| {
                    if v.is_empty() {
                        format!("{name}: nothing.")
                    } else {
                        format!(
                            "{name}:\n{}",
                            v.iter()
                                .map(|i| format!("- {}", i.one_line()))
                                .collect::<Vec<_>>()
                                .join("\n")
                        )
                    }
                };
                let summary = format!(
                    "Now: {} ({}). Today is {} (the day ends at {}).\n{}\n{}\n{}",
                    b.now_local,
                    b.timezone,
                    b.today,
                    b.day_rollover,
                    section("Overdue", &b.overdue),
                    section("Due today", &b.due_today),
                    section("Next 7 days", &b.next_7_days)
                );
                ok(summary, serde_json::to_value(&b).unwrap_or_default())
            }
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Suggest when to work on each open item over the next 7 days, using the user's weekly routine (the blocks marked available \
                       for tasks, see the paperweight://routine resource) and each item's estimated_minutes. Read-only: it changes nothing and \
                       nothing is booked. Deterministic: items are taken in deadline order and each goes, whole, into the earliest available block \
                       that has room and ends before its deadline. Items that fit nowhere before their deadline are returned as `at_risk` with a reason; \
                       raise those with the user first (suggest splitting the work, a longer estimate, or starting now). Items due after this week \
                       that did not fit are in `later`. Takes no parameters.",
        annotations(
            title = "Plan week",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn plan_week(&self) -> ToolResult {
        let res = self
            .with_store(|store, now| {
                let clock = store.clock().map_err(s)?;
                let items = store.open_items().map_err(s)?;
                let routine = store.routine().map_err(s)?;
                Ok(plan_week(now, &items, &routine, &clock))
            })
            .await;
        match res {
            Ok(plan) => {
                let mut summary = String::new();
                if plan.sessions.is_empty() {
                    summary.push_str("No work sessions planned.");
                } else {
                    summary.push_str("Suggested sessions:\n");
                    for sess in &plan.sessions {
                        summary.push_str(&format!(
                            "- {}–{} ({}): {} [{}]\n",
                            sess.start_local,
                            &sess.end_local[sess.end_local.len().saturating_sub(5)..],
                            sess.block_label,
                            sess.title,
                            sess.item_id
                        ));
                    }
                }
                if !plan.at_risk.is_empty() {
                    summary.push_str("\nAt risk:\n");
                    for r in &plan.at_risk {
                        summary.push_str(&format!("- {} [{}]: {}\n", r.title, r.item_id, r.reason));
                    }
                }
                ok(
                    summary.trim_end().to_string(),
                    serde_json::to_value(&plan).unwrap_or_default(),
                )
            }
            Err(e) => fail(e),
        }
    }
}

#[prompt_router]
impl Paperweight {
    #[prompt(
        name = "weekly_review",
        description = "Walk through the coming week with the user: clear overdue items, check what each deadline needs, and agree when to do the work."
    )]
    async fn weekly_review(
        &self,
        Parameters(args): Parameters<WeeklyReviewArgs>,
    ) -> GetPromptResult {
        let focus = args
            .focus
            .filter(|f| !f.trim().is_empty())
            .map(|f| format!("\nFocus on: {f}.\n"))
            .unwrap_or_default();
        let text = format!(
            "Let's do my weekly review in Paperweight. Keep it short and calm — I find long planning sessions hard, so one thing at a time.{focus}

1. Call get_brief and plan_week.
2. Overdue first. For each overdue item, ask me one question: is it done (complete_item), still needed with a new date (update_item), or no longer needed (delete_item)?
3. Then this week's deadlines, earliest first. For each one, read me what's still missing from its requirements and steps, and tick off anything I say is ready (update_item).
4. Anything plan_week marks as at risk: tell me why and suggest one concrete fix (start it today, split it into steps, or move a deadline if that's really possible).
5. Ask if there's anything new I've been meaning to track — paperwork, payments, emails I need to answer — and add it with add_item.
6. Finish with a three-line summary: what's done, what's next, and the one thing to do tomorrow."
        );
        GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)])
            .with_description("Weekly review of Paperweight items")
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for Paperweight {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build(),
        )
        .with_server_info(
            Implementation::new("paperweight", env!("CARGO_PKG_VERSION")).with_title("Paperweight"),
        )
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new(ROUTINE_URI, "routine")
                .with_title("Weekly routine and settings")
                .with_description(
                    "The user's weekly routine blocks (which ones are free for tasks), timezone, day rollover, quiet hours and reminder offsets. \
                     Read this to understand when the user can work and how 'today' is defined.",
                )
                .with_mime_type("application/json"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if request.uri != ROUTINE_URI {
            return Err(McpError::resource_not_found(
                format!(
                    "No resource {}. The only resource is {ROUTINE_URI}.",
                    request.uri
                ),
                None,
            ));
        }
        let res = self
            .with_store(|store, now| {
                let settings = store.settings().map_err(s)?;
                let clock = settings.clock();
                let routine = store.routine().map_err(s)?;
                Ok(json!({
                    "now_local": clock.local(now).to_rfc3339(),
                    "timezone": clock.tz.name(),
                    "today": clock.today(now),
                    "weekday_numbering": "0 = Monday … 6 = Sunday; a block's weekday is the logical day, so a 22:00–02:00 block runs into the next calendar morning",
                    "routine": routine,
                    "settings": settings,
                }))
            })
            .await
            .map_err(|e| McpError::internal_error(e, None))?;
        let text = serde_json::to_string_pretty(&res).unwrap_or_default();
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, ROUTINE_URI).with_mime_type("application/json"),
        ])
        .into())
    }
}
