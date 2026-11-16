# MCP transcript

Every request the test client sent to the real `paperweight-mcp` binary over stdio, with the server's response, captured byte-for-byte from the pipes by `crates/mcp/tests/stdio.rs` (run with `PAPERWEIGHT_TRANSCRIPT=docs/mcp-transcript.md`). Long responses are shown in full.

## 1. initialize

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 0,
  "method": "initialize",
  "params": {
    "protocolVersion": "2026-07-28",
    "capabilities": {},
    "clientInfo": {
      "name": "rmcp",
      "version": "3.10.0"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 0,
  "result": {
    "protocolVersion": "2025-11-25",
    "capabilities": {
      "prompts": {},
      "resources": {},
      "tools": {}
    },
    "serverInfo": {
      "name": "paperweight",
      "title": "Paperweight",
      "version": "0.1.0"
    },
    "instructions": "Paperweight is the user's local deadline tracker for paperwork, assignments, payments and life admin. It reminds them on its own, so your job is to keep it accurate with as little effort from them as possible: when they mention something with a deadline (a form, a bill, an assignment, an appointment), add it with add_item in one go, including any steps and the checklist of what is needed. When they ask what is coming up, what to do today, or how their week looks, call get_brief first. Use plan_week to suggest when to work on things. After adding an item, always confirm it back to the user in one short sentence: the title, the due date and time in their words, and the id. Their day ends at the day_rollover time (usually 02:00), not midnight, so 01:00 on Wednesday counts as Tuesday night. Item ids look like pw-7k3f."
  }
}
```

## 2. notifications/initialized

Request:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/initialized"
}
```

(notification, no response)

## 3. tools/list

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": {
    "_meta": {
      "progressToken": 0
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "add_item",
        "description": "Create a new tracked item (a deadline). Use this whenever the user mentions something they must do by a date: a form to hand in, a bill, an assignment, an appointment, plans with friends. Put everything you know in this one call — steps, the \"what's needed\" checklist (requirements), notes, the source URL or file, a repeat rule — so the user never has to say it twice. Do not use this to change an existing item (use update_item) or to record that something is finished (use complete_item). If the user did not say when it is due, ask them before calling. Returns the full item; afterwards confirm the title, due date and id back to the user. Dates: send ISO 8601 when you can — \"2026-11-20\" means all day (due by the end of that day), \"2026-11-20T17:00\" is a time in the user's timezone, and an offset or Z (\"2026-11-20T08:00Z\") is honoured. Also accepted, and nothing else: today, tonight, tomorrow, a weekday name (the next such day, today included), next <weekday> (the next such day after today), in N days, in N weeks — each optionally followed by a time like 17:00, 5pm or 5:30pm. The user's day ends at 02:00 (the day rollover), so \"today\" at 00:30 is still the previous day, and a time like 01:00 belongs to the night of the given day. Anything else is rejected with an error that states the current local date and time; then convert to ISO 8601 yourself and retry. Never guess a date the user did not give — ask them.",
        "inputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "$defs": {
            "Kind": {
              "description": "What sort of thing an item is.",
              "type": "string",
              "enum": [
                "paperwork",
                "assignment",
                "payment",
                "appointment",
                "social",
                "other"
              ]
            },
            "RecurrenceInput": {
              "type": "object",
              "properties": {
                "freq": {
                  "description": "\"none\", \"daily\", \"weekly\", \"monthly\" or \"yearly\". Monthly items on the\n29th–31st land on the last day of shorter months.",
                  "$ref": "#/$defs/Freq"
                },
                "interval": {
                  "description": "Every N days/weeks/months/years. Default 1.",
                  "type": [
                    "integer",
                    "null"
                  ],
                  "format": "uint32",
                  "minimum": 0,
                  "default": null
                }
              },
              "required": [
                "freq"
              ]
            },
            "Freq": {
              "description": "How often an item repeats.",
              "type": "string",
              "enum": [
                "none",
                "daily",
                "weekly",
                "monthly",
                "yearly"
              ]
            },
            "StepInput": {
              "type": "object",
              "properties": {
                "title": {
                  "description": "What to do, e.g. \"Get a passport photo taken\".",
                  "type": "string"
                },
                "soft_due": {
                  "description": "Optional soft target for this step, same date format as `due`.",
                  "type": [
                    "string",
                    "null"
                  ],
                  "default": null
                }
              },
              "required": [
                "title"
              ]
            }
          },
          "type": "object",
          "properties": {
            "title": {
              "description": "Short name the user would recognise, e.g. \"Renew residence card\". Put details in `notes`.",
              "type": "string"
            },
            "due": {
              "description": "When it is due. See the date rules in the tool description.",
              "type": "string"
            },
            "kind": {
              "description": "paperwork, assignment, payment, appointment, social or other. Default other.",
              "anyOf": [
                {
                  "$ref": "#/$defs/Kind"
                },
                {
                  "type": "null"
                }
              ],
              "default": null
            },
            "notes": {
              "description": "Free-text details: where to go, who to contact, amounts, context.",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "source": {
              "description": "Where it came from: a URL or a local file path (e.g. the PDF of the form).",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "estimated_minutes": {
              "description": "Minutes of work it needs, used by plan_week. Default 30. Use 0 for things that need no preparation (a dinner).",
              "type": [
                "integer",
                "null"
              ],
              "format": "uint32",
              "minimum": 0,
              "default": null
            },
            "recurrence": {
              "description": "Set if it repeats, e.g. {\"freq\": \"monthly\"} for rent.",
              "anyOf": [
                {
                  "$ref": "#/$defs/RecurrenceInput"
                },
                {
                  "type": "null"
                }
              ]
            },
            "steps": {
              "description": "Ordered sub-tasks, if the user mentioned any or they are obvious.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "$ref": "#/$defs/StepInput"
              }
            },
            "requirements": {
              "description": "The \"what's needed\" checklist: documents to bring, forms to fill, where to submit. One line each.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "type": "string"
              },
              "default": null
            }
          },
          "required": [
            "title",
            "due"
          ]
        },
        "annotations": {
          "title": "Add item",
          "readOnlyHint": false,
          "destructiveHint": false,
          "idempotentHint": false,
          "openWorldHint": false
        }
      },
      {
        "name": "complete_item",
        "description": "Mark an item as done. Use this when the user says they finished, submitted, paid or attended it. If the item repeats (e.g. monthly rent), this also creates the next occurrence and returns it as `next`; tell the user the next due date. Do not use this for ticking a single step (use update_item with complete_steps). To undo, call update_item with status \"open\".",
        "inputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "required": [
            "id"
          ],
          "type": "object",
          "properties": {
            "id": {
              "description": "The item id, e.g. \"pw-7k3f\". Call list_items if you do not know it.",
              "type": "string"
            }
          }
        },
        "annotations": {
          "title": "Complete item",
          "readOnlyHint": false,
          "destructiveHint": false,
          "idempotentHint": false,
          "openWorldHint": false
        }
      },
      {
        "name": "delete_item",
        "description": "Delete an item the user no longer needs to track (cancelled, added by mistake, a duplicate). It moves to the archive, where the user can restore it from the Paperweight app for 30 days. Do not use this when the item was finished — use complete_item so it counts as done. Confirm with the user before deleting unless they asked plainly.",
        "inputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "required": [
            "id"
          ],
          "type": "object",
          "properties": {
            "id": {
              "description": "The item id, e.g. \"pw-7k3f\". Call list_items if you do not know it.",
              "type": "string"
            }
          }
        },
        "annotations": {
          "title": "Delete item",
          "readOnlyHint": false,
          "destructiveHint": true,
          "idempotentHint": true,
          "openWorldHint": false
        }
      },
      {
        "name": "get_brief",
        "description": "What needs attention: overdue items, items due today, and items due in the next 7 days, each in priority order (earliest deadline first), plus the current local date and time. Call this first whenever the user asks what is coming up, what to do today or tonight, or how their week looks, and before suggesting what to work on. \"Today\" follows the user's day rollover (02:00 by default), so late-night deadlines count as today. Takes no parameters.",
        "inputSchema": {
          "type": "object",
          "properties": {}
        },
        "annotations": {
          "title": "Get brief",
          "readOnlyHint": true,
          "destructiveHint": false,
          "idempotentHint": true,
          "openWorldHint": false
        }
      },
      {
        "name": "list_items",
        "description": "Find items by status, kind, due-date range and text. Use this to look up an item's id before updating, completing, snoozing or deleting it, or to answer questions like \"what paperwork do I have?\" or \"what did I finish this month?\". For \"what's coming up\" or \"what should I do today\", use get_brief instead. Dates in due_from/due_to take the same formats as add_item's `due`. Results are sorted by deadline.",
        "inputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "$defs": {
            "StatusFilter": {
              "oneOf": [
                {
                  "type": "string",
                  "enum": [
                    "open",
                    "done"
                  ]
                },
                {
                  "description": "Open and done (deleted items are never listed).",
                  "type": "string",
                  "const": "all"
                }
              ]
            },
            "Kind": {
              "description": "What sort of thing an item is.",
              "type": "string",
              "enum": [
                "paperwork",
                "assignment",
                "payment",
                "appointment",
                "social",
                "other"
              ]
            }
          },
          "type": "object",
          "properties": {
            "status": {
              "description": "\"open\" (default), \"done\", or \"all\" (open and done). Deleted items are never listed.",
              "anyOf": [
                {
                  "$ref": "#/$defs/StatusFilter"
                },
                {
                  "type": "null"
                }
              ]
            },
            "kind": {
              "description": "Only this kind.",
              "anyOf": [
                {
                  "$ref": "#/$defs/Kind"
                },
                {
                  "type": "null"
                }
              ],
              "default": null
            },
            "due_from": {
              "description": "Only items due on or after this date (same format as add_item's `due`; a bare date means from the start of that day).",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "due_to": {
              "description": "Only items due on or before this date (a bare date includes that whole day).",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "query": {
              "description": "Case-insensitive text to find in titles, notes, sources, steps and requirements.",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "limit": {
              "description": "Maximum number of items (default 50).",
              "type": [
                "integer",
                "null"
              ],
              "format": "uint",
              "minimum": 0,
              "default": null
            }
          }
        },
        "annotations": {
          "title": "List items",
          "readOnlyHint": true,
          "destructiveHint": false,
          "idempotentHint": true,
          "openWorldHint": false
        }
      },
      {
        "name": "plan_week",
        "description": "Suggest when to work on each open item over the next 7 days, using the user's weekly routine (the blocks marked available for tasks, see the paperweight://routine resource) and each item's estimated_minutes. Read-only: it changes nothing and nothing is booked. Deterministic: items are taken in deadline order and each goes, whole, into the earliest available block that has room and ends before its deadline. Items that fit nowhere before their deadline are returned as `at_risk` with a reason; raise those with the user first (suggest splitting the work, a longer estimate, or starting now). Items due after this week that did not fit are in `later`. Takes no parameters.",
        "inputSchema": {
          "type": "object",
          "properties": {}
        },
        "annotations": {
          "title": "Plan week",
          "readOnlyHint": true,
          "destructiveHint": false,
          "idempotentHint": true,
          "openWorldHint": false
        }
      },
      {
        "name": "snooze_item",
        "description": "Silence an item's reminders until a given time. The deadline does not move. Use this when the user says \"remind me later\", \"not now\" or \"I'll deal with it on Friday\". If the deadline itself changed, use update_item with `due` instead. `until` takes the same formats as add_item's `due` (ISO 8601, or today, tonight, tomorrow, a weekday, next <weekday>, in N days, in N weeks, with an optional time); a date without a time means the start of that day. To resume reminders early, call update_item with clear_snooze true.",
        "inputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "required": [
            "id",
            "until"
          ],
          "type": "object",
          "properties": {
            "id": {
              "description": "The item id, e.g. \"pw-7k3f\".",
              "type": "string"
            },
            "until": {
              "description": "When reminders may resume. Same format as add_item's `due`; a date without a time means the start of that day.",
              "type": "string"
            }
          }
        },
        "annotations": {
          "title": "Snooze item",
          "readOnlyHint": false,
          "destructiveHint": false,
          "idempotentHint": true,
          "openWorldHint": false
        }
      },
      {
        "name": "update_item",
        "description": "Change one existing item: any of its fields, and its steps and requirements (add, tick, untick or remove them by their numeric ids, which appear in the item's `steps` and `requirements`). Only the fields you send change. Use this when the user says a date moved, adds detail, or ticks off part of the work. Do not use this to finish the whole item (use complete_item), to silence reminders (use snooze_item) or to delete it (use delete_item). If you do not know the id, call list_items with a query first. Returns the full item after the change. Dates: send ISO 8601 when you can — \"2026-11-20\" means all day (due by the end of that day), \"2026-11-20T17:00\" is a time in the user's timezone, and an offset or Z (\"2026-11-20T08:00Z\") is honoured. Also accepted, and nothing else: today, tonight, tomorrow, a weekday name (the next such day, today included), next <weekday> (the next such day after today), in N days, in N weeks — each optionally followed by a time like 17:00, 5pm or 5:30pm. The user's day ends at 02:00 (the day rollover), so \"today\" at 00:30 is still the previous day, and a time like 01:00 belongs to the night of the given day. Anything else is rejected with an error that states the current local date and time; then convert to ISO 8601 yourself and retry. Never guess a date the user did not give — ask them.",
        "inputSchema": {
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "$defs": {
            "Kind": {
              "description": "What sort of thing an item is.",
              "type": "string",
              "enum": [
                "paperwork",
                "assignment",
                "payment",
                "appointment",
                "social",
                "other"
              ]
            },
            "RecurrenceInput": {
              "type": "object",
              "properties": {
                "freq": {
                  "description": "\"none\", \"daily\", \"weekly\", \"monthly\" or \"yearly\". Monthly items on the\n29th–31st land on the last day of shorter months.",
                  "$ref": "#/$defs/Freq"
                },
                "interval": {
                  "description": "Every N days/weeks/months/years. Default 1.",
                  "type": [
                    "integer",
                    "null"
                  ],
                  "format": "uint32",
                  "minimum": 0,
                  "default": null
                }
              },
              "required": [
                "freq"
              ]
            },
            "Freq": {
              "description": "How often an item repeats.",
              "type": "string",
              "enum": [
                "none",
                "daily",
                "weekly",
                "monthly",
                "yearly"
              ]
            },
            "Status": {
              "description": "Lifecycle state. `archived` means soft-deleted.",
              "type": "string",
              "enum": [
                "open",
                "done",
                "archived"
              ]
            },
            "StepInput": {
              "type": "object",
              "properties": {
                "title": {
                  "description": "What to do, e.g. \"Get a passport photo taken\".",
                  "type": "string"
                },
                "soft_due": {
                  "description": "Optional soft target for this step, same date format as `due`.",
                  "type": [
                    "string",
                    "null"
                  ],
                  "default": null
                }
              },
              "required": [
                "title"
              ]
            }
          },
          "type": "object",
          "properties": {
            "id": {
              "description": "The item id, e.g. \"pw-7k3f\".",
              "type": "string"
            },
            "title": {
              "description": "New title.",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "due": {
              "description": "New deadline, same format as add_item's `due`. Moving the deadline resets its reminders.",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "kind": {
              "anyOf": [
                {
                  "$ref": "#/$defs/Kind"
                },
                {
                  "type": "null"
                }
              ],
              "default": null
            },
            "notes": {
              "description": "Replaces the notes entirely. To append, send the old notes plus the new text.",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "source": {
              "description": "New source URL or path; an empty string removes it.",
              "type": [
                "string",
                "null"
              ],
              "default": null
            },
            "estimated_minutes": {
              "type": [
                "integer",
                "null"
              ],
              "format": "uint32",
              "minimum": 0,
              "default": null
            },
            "recurrence": {
              "description": "New repeat rule; {\"freq\": \"none\"} stops it repeating.",
              "anyOf": [
                {
                  "$ref": "#/$defs/RecurrenceInput"
                },
                {
                  "type": "null"
                }
              ]
            },
            "status": {
              "description": "\"open\" to reopen a done item. To finish an item use complete_item; to delete use delete_item.",
              "anyOf": [
                {
                  "$ref": "#/$defs/Status"
                },
                {
                  "type": "null"
                }
              ],
              "default": null
            },
            "clear_snooze": {
              "description": "True removes an active snooze so reminders resume.",
              "type": [
                "boolean",
                "null"
              ],
              "default": null
            },
            "add_steps": {
              "description": "Steps to append, in order.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "$ref": "#/$defs/StepInput"
              }
            },
            "complete_steps": {
              "description": "Step ids (numbers from the item's `steps`) to tick off.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "type": "integer",
                "format": "int64"
              },
              "default": null
            },
            "reopen_steps": {
              "description": "Step ids to untick.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "type": "integer",
                "format": "int64"
              },
              "default": null
            },
            "remove_steps": {
              "description": "Step ids to delete.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "type": "integer",
                "format": "int64"
              },
              "default": null
            },
            "add_requirements": {
              "description": "Requirement lines to append to the \"what's needed\" checklist.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "type": "string"
              },
              "default": null
            },
            "check_requirements": {
              "description": "Requirement ids (numbers from the item's `requirements`) to tick off.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "type": "integer",
                "format": "int64"
              },
              "default": null
            },
            "uncheck_requirements": {
              "description": "Requirement ids to untick.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "type": "integer",
                "format": "int64"
              },
              "default": null
            },
            "remove_requirements": {
              "description": "Requirement ids to delete.",
              "type": [
                "array",
                "null"
              ],
              "items": {
                "type": "integer",
                "format": "int64"
              },
              "default": null
            }
          },
          "required": [
            "id"
          ]
        },
        "annotations": {
          "title": "Update item",
          "readOnlyHint": false,
          "destructiveHint": false,
          "idempotentHint": true,
          "openWorldHint": false
        }
      }
    ]
  }
}
```

## 4. tools/call → add_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 1
    },
    "name": "add_item",
    "arguments": {
      "title": "Residence card renewal",
      "due": "in 9 days",
      "kind": "paperwork",
      "notes": "Immigration office, morning",
      "estimated_minutes": 120,
      "steps": [
        {
          "title": "Get photo",
          "soft_due": "in 2 days"
        },
        {
          "title": "Fill in form"
        }
      ],
      "requirements": [
        "Passport",
        "Current card",
        "Photo 4x3"
      ]
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Added pw-fyaw \"Residence card renewal\" — due Wed 25 Nov (all day), paperwork; needs 0/3 ready; steps 0/2 done. Confirm this back to the user."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-fyaw\",\n    \"title\": \"Residence card renewal\",\n    \"kind\": \"paperwork\",\n    \"due_at\": \"2026-11-25T17:00:00Z\",\n    \"all_day\": true,\n    \"status\": \"open\",\n    \"notes\": \"Immigration office, morning\",\n    \"source\": null,\n    \"estimated_minutes\": 120,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"none\",\n      \"interval\": 1\n    },\n    \"steps\": [\n      {\n        \"id\": 1,\n        \"title\": \"Get photo\",\n        \"done\": false,\n        \"soft_due_at\": \"2026-11-18T17:00:00Z\",\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"title\": \"Fill in form\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 1\n      }\n    ],\n    \"requirements\": [\n      {\n        \"id\": 1,\n        \"text\": \"Passport\",\n        \"done\": false,\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"text\": \"Current card\",\n        \"done\": false,\n        \"position\": 1\n      },\n      {\n        \"id\": 3,\n        \"text\": \"Photo 4x3\",\n        \"done\": false,\n        \"position\": 2\n      }\n    ],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2026-11-26T02:00:00+09:00\",\n    \"due_day\": \"2026-11-25\",\n    \"due_label\": \"Wed 25 Nov\",\n    \"overdue\": false,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 0\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-fyaw",
        "title": "Residence card renewal",
        "kind": "paperwork",
        "due_at": "2026-11-25T17:00:00Z",
        "all_day": true,
        "status": "open",
        "notes": "Immigration office, morning",
        "source": null,
        "estimated_minutes": 120,
        "snoozed_until": null,
        "recurrence": {
          "freq": "none",
          "interval": 1
        },
        "steps": [
          {
            "id": 1,
            "title": "Get photo",
            "done": false,
            "soft_due_at": "2026-11-18T17:00:00Z",
            "position": 0
          },
          {
            "id": 2,
            "title": "Fill in form",
            "done": false,
            "soft_due_at": null,
            "position": 1
          }
        ],
        "requirements": [
          {
            "id": 1,
            "text": "Passport",
            "done": false,
            "position": 0
          },
          {
            "id": 2,
            "text": "Current card",
            "done": false,
            "position": 1
          },
          {
            "id": 3,
            "text": "Photo 4x3",
            "done": false,
            "position": 2
          }
        ],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2026-11-26T02:00:00+09:00",
        "due_day": "2026-11-25",
        "due_label": "Wed 25 Nov",
        "overdue": false,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 0
      }
    },
    "isError": false
  }
}
```

## 5. tools/call → add_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 2
    },
    "name": "add_item",
    "arguments": {
      "title": "Rent",
      "due": "2026-01-31",
      "kind": "payment",
      "recurrence": {
        "freq": "monthly"
      },
      "estimated_minutes": 15
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Added pw-fpep \"Rent\" — due overdue (Sat 31 Jan) (all day), payment; repeats monthly. Note: this deadline is already in the past, so it shows as overdue — check the date with the user. Confirm this back to the user."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-fpep\",\n    \"title\": \"Rent\",\n    \"kind\": \"payment\",\n    \"due_at\": \"2026-01-31T17:00:00Z\",\n    \"all_day\": true,\n    \"status\": \"open\",\n    \"notes\": \"\",\n    \"source\": null,\n    \"estimated_minutes\": 15,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"monthly\",\n      \"interval\": 1\n    },\n    \"recurrence_anchor_day\": 31,\n    \"steps\": [],\n    \"requirements\": [],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2026-02-01T02:00:00+09:00\",\n    \"due_day\": \"2026-01-31\",\n    \"due_label\": \"overdue (Sat 31 Jan)\",\n    \"overdue\": true,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 0\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-fpep",
        "title": "Rent",
        "kind": "payment",
        "due_at": "2026-01-31T17:00:00Z",
        "all_day": true,
        "status": "open",
        "notes": "",
        "source": null,
        "estimated_minutes": 15,
        "snoozed_until": null,
        "recurrence": {
          "freq": "monthly",
          "interval": 1
        },
        "recurrence_anchor_day": 31,
        "steps": [],
        "requirements": [],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2026-02-01T02:00:00+09:00",
        "due_day": "2026-01-31",
        "due_label": "overdue (Sat 31 Jan)",
        "overdue": true,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 0
      }
    },
    "isError": false
  }
}
```

## 6. tools/call → add_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 3
    },
    "name": "add_item",
    "arguments": {
      "title": "Late",
      "due": "2020-01-01"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Added pw-jdyu \"Late\" — due overdue (Wed 1 Jan) (all day), other. Note: this deadline is already in the past, so it shows as overdue — check the date with the user. Confirm this back to the user."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-jdyu\",\n    \"title\": \"Late\",\n    \"kind\": \"other\",\n    \"due_at\": \"2020-01-01T17:00:00Z\",\n    \"all_day\": true,\n    \"status\": \"open\",\n    \"notes\": \"\",\n    \"source\": null,\n    \"estimated_minutes\": 30,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"none\",\n      \"interval\": 1\n    },\n    \"steps\": [],\n    \"requirements\": [],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2020-01-02T02:00:00+09:00\",\n    \"due_day\": \"2020-01-01\",\n    \"due_label\": \"overdue (Wed 1 Jan)\",\n    \"overdue\": true,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 0\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-jdyu",
        "title": "Late",
        "kind": "other",
        "due_at": "2020-01-01T17:00:00Z",
        "all_day": true,
        "status": "open",
        "notes": "",
        "source": null,
        "estimated_minutes": 30,
        "snoozed_until": null,
        "recurrence": {
          "freq": "none",
          "interval": 1
        },
        "steps": [],
        "requirements": [],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2020-01-02T02:00:00+09:00",
        "due_day": "2020-01-01",
        "due_label": "overdue (Wed 1 Jan)",
        "overdue": true,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 0
      }
    },
    "isError": false
  }
}
```

## 7. tools/call → add_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 4
    },
    "name": "add_item",
    "arguments": {
      "title": "Thesis chapter",
      "due": "in 3 days",
      "kind": "assignment",
      "estimated_minutes": 600
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Added pw-qgp9 \"Thesis chapter\" — due Thu 19 Nov (all day), assignment. Confirm this back to the user."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-qgp9\",\n    \"title\": \"Thesis chapter\",\n    \"kind\": \"assignment\",\n    \"due_at\": \"2026-11-19T17:00:00Z\",\n    \"all_day\": true,\n    \"status\": \"open\",\n    \"notes\": \"\",\n    \"source\": null,\n    \"estimated_minutes\": 600,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"none\",\n      \"interval\": 1\n    },\n    \"steps\": [],\n    \"requirements\": [],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2026-11-20T02:00:00+09:00\",\n    \"due_day\": \"2026-11-19\",\n    \"due_label\": \"Thu 19 Nov\",\n    \"overdue\": false,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 0\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-qgp9",
        "title": "Thesis chapter",
        "kind": "assignment",
        "due_at": "2026-11-19T17:00:00Z",
        "all_day": true,
        "status": "open",
        "notes": "",
        "source": null,
        "estimated_minutes": 600,
        "snoozed_until": null,
        "recurrence": {
          "freq": "none",
          "interval": 1
        },
        "steps": [],
        "requirements": [],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2026-11-20T02:00:00+09:00",
        "due_day": "2026-11-19",
        "due_label": "Thu 19 Nov",
        "overdue": false,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 0
      }
    },
    "isError": false
  }
}
```

## 8. tools/call → add_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 5
    },
    "name": "add_item",
    "arguments": {
      "title": "Form",
      "due": "end of the month"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "due: Could not understand the date \"end of the month\". It is now Monday 2026-11-16 23:38 (Asia/Tokyo); the current day ends at 02:00 so \"today\" is Monday 2026-11-16. Send the date in ISO 8601 instead, for example \"2026-11-17\" for an all-day deadline or \"2026-11-17T17:00\" for a specific time (no offset means Asia/Tokyo time). Also accepted: today, tonight, tomorrow, a weekday name, next <weekday>, in N days, in N weeks, each optionally followed by a time such as 17:00 or 5pm."
      }
    ],
    "isError": true
  }
}
```

## 9. tools/call → add_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 6
    },
    "name": "add_item",
    "arguments": {
      "title": "   ",
      "due": "tomorrow"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "title must not be empty"
      }
    ],
    "isError": true
  }
}
```

## 10. tools/call → add_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 7
    },
    "name": "add_item",
    "arguments": {
      "title": "x",
      "due": "tomorrow",
      "recurrence": {
        "freq": "weekly",
        "interval": 0
      }
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "recurrence.interval must be between 1 and 366 (1 means every day/week/month/year)."
      }
    ],
    "isError": true
  }
}
```

## 11. tools/call → add_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 9,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 8
    },
    "name": "add_item",
    "arguments": {
      "title": 5
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 9,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "failed to deserialize parameters: invalid type: integer `5`, expected a string"
      }
    ],
    "isError": true
  }
}
```

## 12. tools/call → no_such_tool

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 9
    },
    "name": "no_such_tool"
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "error": {
    "code": -32602,
    "message": "tool not found"
  }
}
```

## 13. tools/call → update_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 10
    },
    "name": "update_item",
    "arguments": {
      "id": "pw-fyaw",
      "due": "in 10 days 10:00",
      "complete_steps": [
        1
      ],
      "add_steps": [
        {
          "title": "Book appointment"
        }
      ],
      "check_requirements": [
        1
      ],
      "remove_requirements": [
        3
      ],
      "add_requirements": [
        "Revenue stamp"
      ],
      "source": "https://example.org/form.pdf"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Updated pw-fyaw \"Residence card renewal\" — due Thu 26 Nov 10:00, paperwork; needs 1/3 ready; steps 1/3 done."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-fyaw\",\n    \"title\": \"Residence card renewal\",\n    \"kind\": \"paperwork\",\n    \"due_at\": \"2026-11-26T01:00:00Z\",\n    \"all_day\": false,\n    \"status\": \"open\",\n    \"notes\": \"Immigration office, morning\",\n    \"source\": \"https://example.org/form.pdf\",\n    \"estimated_minutes\": 120,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"none\",\n      \"interval\": 1\n    },\n    \"steps\": [\n      {\n        \"id\": 1,\n        \"title\": \"Get photo\",\n        \"done\": true,\n        \"soft_due_at\": \"2026-11-18T17:00:00Z\",\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"title\": \"Fill in form\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 1\n      },\n      {\n        \"id\": 3,\n        \"title\": \"Book appointment\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 2\n      }\n    ],\n    \"requirements\": [\n      {\n        \"id\": 1,\n        \"text\": \"Passport\",\n        \"done\": true,\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"text\": \"Current card\",\n        \"done\": false,\n        \"position\": 1\n      },\n      {\n        \"id\": 4,\n        \"text\": \"Revenue stamp\",\n        \"done\": false,\n        \"position\": 2\n      }\n    ],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2026-11-26T10:00:00+09:00\",\n    \"due_day\": \"2026-11-26\",\n    \"due_label\": \"Thu 26 Nov 10:00\",\n    \"overdue\": false,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 1,\n    \"requirements_done\": 1\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-fyaw",
        "title": "Residence card renewal",
        "kind": "paperwork",
        "due_at": "2026-11-26T01:00:00Z",
        "all_day": false,
        "status": "open",
        "notes": "Immigration office, morning",
        "source": "https://example.org/form.pdf",
        "estimated_minutes": 120,
        "snoozed_until": null,
        "recurrence": {
          "freq": "none",
          "interval": 1
        },
        "steps": [
          {
            "id": 1,
            "title": "Get photo",
            "done": true,
            "soft_due_at": "2026-11-18T17:00:00Z",
            "position": 0
          },
          {
            "id": 2,
            "title": "Fill in form",
            "done": false,
            "soft_due_at": null,
            "position": 1
          },
          {
            "id": 3,
            "title": "Book appointment",
            "done": false,
            "soft_due_at": null,
            "position": 2
          }
        ],
        "requirements": [
          {
            "id": 1,
            "text": "Passport",
            "done": true,
            "position": 0
          },
          {
            "id": 2,
            "text": "Current card",
            "done": false,
            "position": 1
          },
          {
            "id": 4,
            "text": "Revenue stamp",
            "done": false,
            "position": 2
          }
        ],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2026-11-26T10:00:00+09:00",
        "due_day": "2026-11-26",
        "due_label": "Thu 26 Nov 10:00",
        "overdue": false,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 1,
        "requirements_done": 1
      }
    },
    "isError": false
  }
}
```

## 14. tools/call → update_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 11
    },
    "name": "update_item",
    "arguments": {
      "id": "pw-fyaw",
      "source": "",
      "reopen_steps": [
        1
      ]
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Updated pw-fyaw \"Residence card renewal\" — due Thu 26 Nov 10:00, paperwork; needs 1/3 ready; steps 0/3 done."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-fyaw\",\n    \"title\": \"Residence card renewal\",\n    \"kind\": \"paperwork\",\n    \"due_at\": \"2026-11-26T01:00:00Z\",\n    \"all_day\": false,\n    \"status\": \"open\",\n    \"notes\": \"Immigration office, morning\",\n    \"source\": null,\n    \"estimated_minutes\": 120,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"none\",\n      \"interval\": 1\n    },\n    \"steps\": [\n      {\n        \"id\": 1,\n        \"title\": \"Get photo\",\n        \"done\": false,\n        \"soft_due_at\": \"2026-11-18T17:00:00Z\",\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"title\": \"Fill in form\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 1\n      },\n      {\n        \"id\": 3,\n        \"title\": \"Book appointment\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 2\n      }\n    ],\n    \"requirements\": [\n      {\n        \"id\": 1,\n        \"text\": \"Passport\",\n        \"done\": true,\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"text\": \"Current card\",\n        \"done\": false,\n        \"position\": 1\n      },\n      {\n        \"id\": 4,\n        \"text\": \"Revenue stamp\",\n        \"done\": false,\n        \"position\": 2\n      }\n    ],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2026-11-26T10:00:00+09:00\",\n    \"due_day\": \"2026-11-26\",\n    \"due_label\": \"Thu 26 Nov 10:00\",\n    \"overdue\": false,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 1\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-fyaw",
        "title": "Residence card renewal",
        "kind": "paperwork",
        "due_at": "2026-11-26T01:00:00Z",
        "all_day": false,
        "status": "open",
        "notes": "Immigration office, morning",
        "source": null,
        "estimated_minutes": 120,
        "snoozed_until": null,
        "recurrence": {
          "freq": "none",
          "interval": 1
        },
        "steps": [
          {
            "id": 1,
            "title": "Get photo",
            "done": false,
            "soft_due_at": "2026-11-18T17:00:00Z",
            "position": 0
          },
          {
            "id": 2,
            "title": "Fill in form",
            "done": false,
            "soft_due_at": null,
            "position": 1
          },
          {
            "id": 3,
            "title": "Book appointment",
            "done": false,
            "soft_due_at": null,
            "position": 2
          }
        ],
        "requirements": [
          {
            "id": 1,
            "text": "Passport",
            "done": true,
            "position": 0
          },
          {
            "id": 2,
            "text": "Current card",
            "done": false,
            "position": 1
          },
          {
            "id": 4,
            "text": "Revenue stamp",
            "done": false,
            "position": 2
          }
        ],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2026-11-26T10:00:00+09:00",
        "due_day": "2026-11-26",
        "due_label": "Thu 26 Nov 10:00",
        "overdue": false,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 1
      }
    },
    "isError": false
  }
}
```

## 15. tools/call → update_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 13,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 12
    },
    "name": "update_item",
    "arguments": {
      "id": "pw-xxxx",
      "title": "y"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 13,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "No item with id pw-xxxx. Call list_items to find it."
      }
    ],
    "isError": true
  }
}
```

## 16. tools/call → update_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 14,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 13
    },
    "name": "update_item",
    "arguments": {
      "id": "pw-fyaw",
      "complete_steps": [
        99999
      ]
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 14,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Item pw-fyaw has no step with id 99999. Its steps are: 1 \"Get photo\", 2 \"Fill in form\", 3 \"Book appointment\"."
      }
    ],
    "isError": true
  }
}
```

## 17. tools/call → update_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 15,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 14
    },
    "name": "update_item",
    "arguments": {
      "id": "pw-fyaw",
      "due": "someday"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 15,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "due: Could not understand the date \"someday\". It is now Monday 2026-11-16 23:38 (Asia/Tokyo); the current day ends at 02:00 so \"today\" is Monday 2026-11-16. Send the date in ISO 8601 instead, for example \"2026-11-17\" for an all-day deadline or \"2026-11-17T17:00\" for a specific time (no offset means Asia/Tokyo time). Also accepted: today, tonight, tomorrow, a weekday name, next <weekday>, in N days, in N weeks, each optionally followed by a time such as 17:00 or 5pm."
      }
    ],
    "isError": true
  }
}
```

## 18. tools/call → update_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 16,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 15
    },
    "name": "update_item",
    "arguments": {
      "id": "pw-fyaw",
      "status": "archived"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 16,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "To delete an item use delete_item. update_item accepts status \"open\" or \"done\"."
      }
    ],
    "isError": true
  }
}
```

## 19. tools/call → snooze_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 17,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 16
    },
    "name": "snooze_item",
    "arguments": {
      "id": "pw-fyaw",
      "until": "tomorrow 15:00"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 17,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Snoozed reminders for pw-fyaw \"Residence card renewal\" until tomorrow 15:00. The deadline is unchanged: Thu 26 Nov 10:00."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-fyaw\",\n    \"title\": \"Residence card renewal\",\n    \"kind\": \"paperwork\",\n    \"due_at\": \"2026-11-26T01:00:00Z\",\n    \"all_day\": false,\n    \"status\": \"open\",\n    \"notes\": \"Immigration office, morning\",\n    \"source\": null,\n    \"estimated_minutes\": 120,\n    \"snoozed_until\": \"2026-11-17T06:00:00Z\",\n    \"recurrence\": {\n      \"freq\": \"none\",\n      \"interval\": 1\n    },\n    \"steps\": [\n      {\n        \"id\": 1,\n        \"title\": \"Get photo\",\n        \"done\": false,\n        \"soft_due_at\": \"2026-11-18T17:00:00Z\",\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"title\": \"Fill in form\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 1\n      },\n      {\n        \"id\": 3,\n        \"title\": \"Book appointment\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 2\n      }\n    ],\n    \"requirements\": [\n      {\n        \"id\": 1,\n        \"text\": \"Passport\",\n        \"done\": true,\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"text\": \"Current card\",\n        \"done\": false,\n        \"position\": 1\n      },\n      {\n        \"id\": 4,\n        \"text\": \"Revenue stamp\",\n        \"done\": false,\n        \"position\": 2\n      }\n    ],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2026-11-26T10:00:00+09:00\",\n    \"due_day\": \"2026-11-26\",\n    \"due_label\": \"Thu 26 Nov 10:00\",\n    \"overdue\": false,\n    \"snoozed\": true,\n    \"snoozed_until_label\": \"tomorrow 15:00\",\n    \"steps_done\": 0,\n    \"requirements_done\": 1\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-fyaw",
        "title": "Residence card renewal",
        "kind": "paperwork",
        "due_at": "2026-11-26T01:00:00Z",
        "all_day": false,
        "status": "open",
        "notes": "Immigration office, morning",
        "source": null,
        "estimated_minutes": 120,
        "snoozed_until": "2026-11-17T06:00:00Z",
        "recurrence": {
          "freq": "none",
          "interval": 1
        },
        "steps": [
          {
            "id": 1,
            "title": "Get photo",
            "done": false,
            "soft_due_at": "2026-11-18T17:00:00Z",
            "position": 0
          },
          {
            "id": 2,
            "title": "Fill in form",
            "done": false,
            "soft_due_at": null,
            "position": 1
          },
          {
            "id": 3,
            "title": "Book appointment",
            "done": false,
            "soft_due_at": null,
            "position": 2
          }
        ],
        "requirements": [
          {
            "id": 1,
            "text": "Passport",
            "done": true,
            "position": 0
          },
          {
            "id": 2,
            "text": "Current card",
            "done": false,
            "position": 1
          },
          {
            "id": 4,
            "text": "Revenue stamp",
            "done": false,
            "position": 2
          }
        ],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2026-11-26T10:00:00+09:00",
        "due_day": "2026-11-26",
        "due_label": "Thu 26 Nov 10:00",
        "overdue": false,
        "snoozed": true,
        "snoozed_until_label": "tomorrow 15:00",
        "steps_done": 0,
        "requirements_done": 1
      }
    },
    "isError": false
  }
}
```

## 20. tools/call → snooze_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 18,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 17
    },
    "name": "snooze_item",
    "arguments": {
      "id": "pw-fyaw",
      "until": "2020-01-01T10:00"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 18,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "The snooze time is in the past. Give a time after now; snooze only silences reminders and does not move the deadline."
      }
    ],
    "isError": true
  }
}
```

## 21. tools/call → snooze_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 19,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 18
    },
    "name": "snooze_item",
    "arguments": {
      "id": "pw-zzzz",
      "until": "tomorrow"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 19,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "No item with id pw-zzzz. Call list_items to find it."
      }
    ],
    "isError": true
  }
}
```

## 22. tools/call → update_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 20,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 19
    },
    "name": "update_item",
    "arguments": {
      "id": "pw-fyaw",
      "clear_snooze": true
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 20,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Updated pw-fyaw \"Residence card renewal\" — due Thu 26 Nov 10:00, paperwork; needs 1/3 ready; steps 0/3 done."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-fyaw\",\n    \"title\": \"Residence card renewal\",\n    \"kind\": \"paperwork\",\n    \"due_at\": \"2026-11-26T01:00:00Z\",\n    \"all_day\": false,\n    \"status\": \"open\",\n    \"notes\": \"Immigration office, morning\",\n    \"source\": null,\n    \"estimated_minutes\": 120,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"none\",\n      \"interval\": 1\n    },\n    \"steps\": [\n      {\n        \"id\": 1,\n        \"title\": \"Get photo\",\n        \"done\": false,\n        \"soft_due_at\": \"2026-11-18T17:00:00Z\",\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"title\": \"Fill in form\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 1\n      },\n      {\n        \"id\": 3,\n        \"title\": \"Book appointment\",\n        \"done\": false,\n        \"soft_due_at\": null,\n        \"position\": 2\n      }\n    ],\n    \"requirements\": [\n      {\n        \"id\": 1,\n        \"text\": \"Passport\",\n        \"done\": true,\n        \"position\": 0\n      },\n      {\n        \"id\": 2,\n        \"text\": \"Current card\",\n        \"done\": false,\n        \"position\": 1\n      },\n      {\n        \"id\": 4,\n        \"text\": \"Revenue stamp\",\n        \"done\": false,\n        \"position\": 2\n      }\n    ],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2026-11-26T10:00:00+09:00\",\n    \"due_day\": \"2026-11-26\",\n    \"due_label\": \"Thu 26 Nov 10:00\",\n    \"overdue\": false,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 1\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-fyaw",
        "title": "Residence card renewal",
        "kind": "paperwork",
        "due_at": "2026-11-26T01:00:00Z",
        "all_day": false,
        "status": "open",
        "notes": "Immigration office, morning",
        "source": null,
        "estimated_minutes": 120,
        "snoozed_until": null,
        "recurrence": {
          "freq": "none",
          "interval": 1
        },
        "steps": [
          {
            "id": 1,
            "title": "Get photo",
            "done": false,
            "soft_due_at": "2026-11-18T17:00:00Z",
            "position": 0
          },
          {
            "id": 2,
            "title": "Fill in form",
            "done": false,
            "soft_due_at": null,
            "position": 1
          },
          {
            "id": 3,
            "title": "Book appointment",
            "done": false,
            "soft_due_at": null,
            "position": 2
          }
        ],
        "requirements": [
          {
            "id": 1,
            "text": "Passport",
            "done": true,
            "position": 0
          },
          {
            "id": 2,
            "text": "Current card",
            "done": false,
            "position": 1
          },
          {
            "id": 4,
            "text": "Revenue stamp",
            "done": false,
            "position": 2
          }
        ],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2026-11-26T10:00:00+09:00",
        "due_day": "2026-11-26",
        "due_label": "Thu 26 Nov 10:00",
        "overdue": false,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 1
      }
    },
    "isError": false
  }
}
```

## 23. tools/call → complete_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 21,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 20
    },
    "name": "complete_item",
    "arguments": {
      "id": "pw-fpep"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 21,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Marked pw-fpep \"Rent\" as done. It repeats; the next one is pw-jppg \"Rent\" — due Mon 30 Nov (all day), payment; repeats monthly."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-fpep\",\n    \"title\": \"Rent\",\n    \"kind\": \"payment\",\n    \"due_at\": \"2026-01-31T17:00:00Z\",\n    \"all_day\": true,\n    \"status\": \"done\",\n    \"notes\": \"\",\n    \"source\": null,\n    \"estimated_minutes\": 15,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"monthly\",\n      \"interval\": 1\n    },\n    \"recurrence_anchor_day\": 31,\n    \"steps\": [],\n    \"requirements\": [],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": \"2026-11-16T14:38:22Z\",\n    \"archived_at\": null,\n    \"due_local\": \"2026-02-01T02:00:00+09:00\",\n    \"due_day\": \"2026-01-31\",\n    \"due_label\": \"Sat 31 Jan\",\n    \"overdue\": false,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 0\n  },\n  \"next\": {\n    \"id\": \"pw-jppg\",\n    \"title\": \"Rent\",\n    \"kind\": \"payment\",\n    \"due_at\": \"2026-11-30T17:00:00Z\",\n    \"all_day\": true,\n    \"status\": \"open\",\n    \"notes\": \"\",\n    \"source\": null,\n    \"estimated_minutes\": 15,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"monthly\",\n      \"interval\": 1\n    },\n    \"recurrence_anchor_day\": 31,\n    \"steps\": [],\n    \"requirements\": [],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": null,\n    \"due_local\": \"2026-12-01T02:00:00+09:00\",\n    \"due_day\": \"2026-11-30\",\n    \"due_label\": \"Mon 30 Nov\",\n    \"overdue\": false,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 0\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-fpep",
        "title": "Rent",
        "kind": "payment",
        "due_at": "2026-01-31T17:00:00Z",
        "all_day": true,
        "status": "done",
        "notes": "",
        "source": null,
        "estimated_minutes": 15,
        "snoozed_until": null,
        "recurrence": {
          "freq": "monthly",
          "interval": 1
        },
        "recurrence_anchor_day": 31,
        "steps": [],
        "requirements": [],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": "2026-11-16T14:38:22Z",
        "archived_at": null,
        "due_local": "2026-02-01T02:00:00+09:00",
        "due_day": "2026-01-31",
        "due_label": "Sat 31 Jan",
        "overdue": false,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 0
      },
      "next": {
        "id": "pw-jppg",
        "title": "Rent",
        "kind": "payment",
        "due_at": "2026-11-30T17:00:00Z",
        "all_day": true,
        "status": "open",
        "notes": "",
        "source": null,
        "estimated_minutes": 15,
        "snoozed_until": null,
        "recurrence": {
          "freq": "monthly",
          "interval": 1
        },
        "recurrence_anchor_day": 31,
        "steps": [],
        "requirements": [],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": null,
        "due_local": "2026-12-01T02:00:00+09:00",
        "due_day": "2026-11-30",
        "due_label": "Mon 30 Nov",
        "overdue": false,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 0
      }
    },
    "isError": false
  }
}
```

## 24. tools/call → complete_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 22,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 21
    },
    "name": "complete_item",
    "arguments": {
      "id": "pw-fpep"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 22,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Item pw-fpep is already done (completed 2026-11-16T14:38:22Z). To reopen it, call update_item with status \"open\"."
      }
    ],
    "isError": true
  }
}
```

## 25. tools/call → complete_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 23,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 22
    },
    "name": "complete_item",
    "arguments": {
      "id": "pw-nope"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 23,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "No item with id pw-nope. Call list_items to find it."
      }
    ],
    "isError": true
  }
}
```

## 26. tools/call → list_items

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 24,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 23
    },
    "name": "list_items",
    "arguments": {}
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 24,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "4 items:\n- pw-jdyu \"Late\" — overdue (Wed 1 Jan) (other)\n- pw-qgp9 \"Thesis chapter\" — Thu 19 Nov (assignment)\n- pw-fyaw \"Residence card renewal\" — Thu 26 Nov 10:00 (paperwork)\n- pw-jppg \"Rent\" — Mon 30 Nov (payment)"
      },
      {
        "type": "text",
        "text": "{\n  \"count\": 4,\n  \"items\": [\n    {\n      \"id\": \"pw-jdyu\",\n      \"title\": \"Late\",\n      \"kind\": \"other\",\n      \"due_at\": \"2020-01-01T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 30,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2020-01-02T02:00:00+09:00\",\n      \"due_day\": \"2020-01-01\",\n      \"due_label\": \"overdue (Wed 1 Jan)\",\n      \"overdue\": true,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    },\n    {\n      \"id\": \"pw-qgp9\",\n      \"title\": \"Thesis chapter\",\n      \"kind\": \"assignment\",\n      \"due_at\": \"2026-11-19T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 600,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-11-20T02:00:00+09:00\",\n      \"due_day\": \"2026-11-19\",\n      \"due_label\": \"Thu 19 Nov\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    },\n    {\n      \"id\": \"pw-fyaw\",\n      \"title\": \"Residence card renewal\",\n      \"kind\": \"paperwork\",\n      \"due_at\": \"2026-11-26T01:00:00Z\",\n      \"all_day\": false,\n      \"status\": \"open\",\n      \"notes\": \"Immigration office, morning\",\n      \"source\": null,\n      \"estimated_minutes\": 120,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [\n        {\n          \"id\": 1,\n          \"title\": \"Get photo\",\n          \"done\": false,\n          \"soft_due_at\": \"2026-11-18T17:00:00Z\",\n          \"position\": 0\n        },\n        {\n          \"id\": 2,\n          \"title\": \"Fill in form\",\n          \"done\": false,\n          \"soft_due_at\": null,\n          \"position\": 1\n        },\n        {\n          \"id\": 3,\n          \"title\": \"Book appointment\",\n          \"done\": false,\n          \"soft_due_at\": null,\n          \"position\": 2\n        }\n      ],\n      \"requirements\": [\n        {\n          \"id\": 1,\n          \"text\": \"Passport\",\n          \"done\": true,\n          \"position\": 0\n        },\n        {\n          \"id\": 2,\n          \"text\": \"Current card\",\n          \"done\": false,\n          \"position\": 1\n        },\n        {\n          \"id\": 4,\n          \"text\": \"Revenue stamp\",\n          \"done\": false,\n          \"position\": 2\n        }\n      ],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-11-26T10:00:00+09:00\",\n      \"due_day\": \"2026-11-26\",\n      \"due_label\": \"Thu 26 Nov 10:00\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 1\n    },\n    {\n      \"id\": \"pw-jppg\",\n      \"title\": \"Rent\",\n      \"kind\": \"payment\",\n      \"due_at\": \"2026-11-30T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 15,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"monthly\",\n        \"interval\": 1\n      },\n      \"recurrence_anchor_day\": 31,\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-12-01T02:00:00+09:00\",\n      \"due_day\": \"2026-11-30\",\n      \"due_label\": \"Mon 30 Nov\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    }\n  ]\n}"
      }
    ],
    "structuredContent": {
      "count": 4,
      "items": [
        {
          "id": "pw-jdyu",
          "title": "Late",
          "kind": "other",
          "due_at": "2020-01-01T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 30,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2020-01-02T02:00:00+09:00",
          "due_day": "2020-01-01",
          "due_label": "overdue (Wed 1 Jan)",
          "overdue": true,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        },
        {
          "id": "pw-qgp9",
          "title": "Thesis chapter",
          "kind": "assignment",
          "due_at": "2026-11-19T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 600,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-11-20T02:00:00+09:00",
          "due_day": "2026-11-19",
          "due_label": "Thu 19 Nov",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        },
        {
          "id": "pw-fyaw",
          "title": "Residence card renewal",
          "kind": "paperwork",
          "due_at": "2026-11-26T01:00:00Z",
          "all_day": false,
          "status": "open",
          "notes": "Immigration office, morning",
          "source": null,
          "estimated_minutes": 120,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [
            {
              "id": 1,
              "title": "Get photo",
              "done": false,
              "soft_due_at": "2026-11-18T17:00:00Z",
              "position": 0
            },
            {
              "id": 2,
              "title": "Fill in form",
              "done": false,
              "soft_due_at": null,
              "position": 1
            },
            {
              "id": 3,
              "title": "Book appointment",
              "done": false,
              "soft_due_at": null,
              "position": 2
            }
          ],
          "requirements": [
            {
              "id": 1,
              "text": "Passport",
              "done": true,
              "position": 0
            },
            {
              "id": 2,
              "text": "Current card",
              "done": false,
              "position": 1
            },
            {
              "id": 4,
              "text": "Revenue stamp",
              "done": false,
              "position": 2
            }
          ],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-11-26T10:00:00+09:00",
          "due_day": "2026-11-26",
          "due_label": "Thu 26 Nov 10:00",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 1
        },
        {
          "id": "pw-jppg",
          "title": "Rent",
          "kind": "payment",
          "due_at": "2026-11-30T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 15,
          "snoozed_until": null,
          "recurrence": {
            "freq": "monthly",
            "interval": 1
          },
          "recurrence_anchor_day": 31,
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-12-01T02:00:00+09:00",
          "due_day": "2026-11-30",
          "due_label": "Mon 30 Nov",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        }
      ]
    },
    "isError": false
  }
}
```

## 27. tools/call → list_items

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 25,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 24
    },
    "name": "list_items",
    "arguments": {
      "query": "revenue STAMP"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 25,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "1 item:\n- pw-fyaw \"Residence card renewal\" — Thu 26 Nov 10:00 (paperwork)"
      },
      {
        "type": "text",
        "text": "{\n  \"count\": 1,\n  \"items\": [\n    {\n      \"id\": \"pw-fyaw\",\n      \"title\": \"Residence card renewal\",\n      \"kind\": \"paperwork\",\n      \"due_at\": \"2026-11-26T01:00:00Z\",\n      \"all_day\": false,\n      \"status\": \"open\",\n      \"notes\": \"Immigration office, morning\",\n      \"source\": null,\n      \"estimated_minutes\": 120,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [\n        {\n          \"id\": 1,\n          \"title\": \"Get photo\",\n          \"done\": false,\n          \"soft_due_at\": \"2026-11-18T17:00:00Z\",\n          \"position\": 0\n        },\n        {\n          \"id\": 2,\n          \"title\": \"Fill in form\",\n          \"done\": false,\n          \"soft_due_at\": null,\n          \"position\": 1\n        },\n        {\n          \"id\": 3,\n          \"title\": \"Book appointment\",\n          \"done\": false,\n          \"soft_due_at\": null,\n          \"position\": 2\n        }\n      ],\n      \"requirements\": [\n        {\n          \"id\": 1,\n          \"text\": \"Passport\",\n          \"done\": true,\n          \"position\": 0\n        },\n        {\n          \"id\": 2,\n          \"text\": \"Current card\",\n          \"done\": false,\n          \"position\": 1\n        },\n        {\n          \"id\": 4,\n          \"text\": \"Revenue stamp\",\n          \"done\": false,\n          \"position\": 2\n        }\n      ],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-11-26T10:00:00+09:00\",\n      \"due_day\": \"2026-11-26\",\n      \"due_label\": \"Thu 26 Nov 10:00\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 1\n    }\n  ]\n}"
      }
    ],
    "structuredContent": {
      "count": 1,
      "items": [
        {
          "id": "pw-fyaw",
          "title": "Residence card renewal",
          "kind": "paperwork",
          "due_at": "2026-11-26T01:00:00Z",
          "all_day": false,
          "status": "open",
          "notes": "Immigration office, morning",
          "source": null,
          "estimated_minutes": 120,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [
            {
              "id": 1,
              "title": "Get photo",
              "done": false,
              "soft_due_at": "2026-11-18T17:00:00Z",
              "position": 0
            },
            {
              "id": 2,
              "title": "Fill in form",
              "done": false,
              "soft_due_at": null,
              "position": 1
            },
            {
              "id": 3,
              "title": "Book appointment",
              "done": false,
              "soft_due_at": null,
              "position": 2
            }
          ],
          "requirements": [
            {
              "id": 1,
              "text": "Passport",
              "done": true,
              "position": 0
            },
            {
              "id": 2,
              "text": "Current card",
              "done": false,
              "position": 1
            },
            {
              "id": 4,
              "text": "Revenue stamp",
              "done": false,
              "position": 2
            }
          ],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-11-26T10:00:00+09:00",
          "due_day": "2026-11-26",
          "due_label": "Thu 26 Nov 10:00",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 1
        }
      ]
    },
    "isError": false
  }
}
```

## 28. tools/call → list_items

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 26,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 25
    },
    "name": "list_items",
    "arguments": {
      "kind": "payment",
      "status": "all"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 26,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "2 items:\n- pw-fpep \"Rent\" — Sat 31 Jan (payment)\n- pw-jppg \"Rent\" — Mon 30 Nov (payment)"
      },
      {
        "type": "text",
        "text": "{\n  \"count\": 2,\n  \"items\": [\n    {\n      \"id\": \"pw-fpep\",\n      \"title\": \"Rent\",\n      \"kind\": \"payment\",\n      \"due_at\": \"2026-01-31T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"done\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 15,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"monthly\",\n        \"interval\": 1\n      },\n      \"recurrence_anchor_day\": 31,\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": \"2026-11-16T14:38:22Z\",\n      \"archived_at\": null,\n      \"due_local\": \"2026-02-01T02:00:00+09:00\",\n      \"due_day\": \"2026-01-31\",\n      \"due_label\": \"Sat 31 Jan\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    },\n    {\n      \"id\": \"pw-jppg\",\n      \"title\": \"Rent\",\n      \"kind\": \"payment\",\n      \"due_at\": \"2026-11-30T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 15,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"monthly\",\n        \"interval\": 1\n      },\n      \"recurrence_anchor_day\": 31,\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-12-01T02:00:00+09:00\",\n      \"due_day\": \"2026-11-30\",\n      \"due_label\": \"Mon 30 Nov\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    }\n  ]\n}"
      }
    ],
    "structuredContent": {
      "count": 2,
      "items": [
        {
          "id": "pw-fpep",
          "title": "Rent",
          "kind": "payment",
          "due_at": "2026-01-31T17:00:00Z",
          "all_day": true,
          "status": "done",
          "notes": "",
          "source": null,
          "estimated_minutes": 15,
          "snoozed_until": null,
          "recurrence": {
            "freq": "monthly",
            "interval": 1
          },
          "recurrence_anchor_day": 31,
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": "2026-11-16T14:38:22Z",
          "archived_at": null,
          "due_local": "2026-02-01T02:00:00+09:00",
          "due_day": "2026-01-31",
          "due_label": "Sat 31 Jan",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        },
        {
          "id": "pw-jppg",
          "title": "Rent",
          "kind": "payment",
          "due_at": "2026-11-30T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 15,
          "snoozed_until": null,
          "recurrence": {
            "freq": "monthly",
            "interval": 1
          },
          "recurrence_anchor_day": 31,
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-12-01T02:00:00+09:00",
          "due_day": "2026-11-30",
          "due_label": "Mon 30 Nov",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        }
      ]
    },
    "isError": false
  }
}
```

## 29. tools/call → list_items

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 27,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 26
    },
    "name": "list_items",
    "arguments": {
      "due_from": "tomorrow",
      "due_to": "in 5 days"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 27,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "1 item:\n- pw-qgp9 \"Thesis chapter\" — Thu 19 Nov (assignment)"
      },
      {
        "type": "text",
        "text": "{\n  \"count\": 1,\n  \"items\": [\n    {\n      \"id\": \"pw-qgp9\",\n      \"title\": \"Thesis chapter\",\n      \"kind\": \"assignment\",\n      \"due_at\": \"2026-11-19T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 600,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-11-20T02:00:00+09:00\",\n      \"due_day\": \"2026-11-19\",\n      \"due_label\": \"Thu 19 Nov\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    }\n  ]\n}"
      }
    ],
    "structuredContent": {
      "count": 1,
      "items": [
        {
          "id": "pw-qgp9",
          "title": "Thesis chapter",
          "kind": "assignment",
          "due_at": "2026-11-19T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 600,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-11-20T02:00:00+09:00",
          "due_day": "2026-11-19",
          "due_label": "Thu 19 Nov",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        }
      ]
    },
    "isError": false
  }
}
```

## 30. tools/call → list_items

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 28,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 27
    },
    "name": "list_items",
    "arguments": {
      "due_from": "last week"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 28,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "due_from: Could not understand the date \"last week\". It is now Monday 2026-11-16 23:38 (Asia/Tokyo); the current day ends at 02:00 so \"today\" is Monday 2026-11-16. Send the date in ISO 8601 instead, for example \"2026-11-17\" for an all-day deadline or \"2026-11-17T17:00\" for a specific time (no offset means Asia/Tokyo time). Also accepted: today, tonight, tomorrow, a weekday name, next <weekday>, in N days, in N weeks, each optionally followed by a time such as 17:00 or 5pm."
      }
    ],
    "isError": true
  }
}
```

## 31. tools/call → get_brief

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 29,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 28
    },
    "name": "get_brief"
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 29,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Now: 2026-11-16T23:38:22+09:00 (Asia/Tokyo). Today is 2026-11-16 (the day ends at 02:00).\nOverdue:\n- pw-jdyu \"Late\" — overdue (Wed 1 Jan) (other)\nDue today: nothing.\nNext 7 days:\n- pw-qgp9 \"Thesis chapter\" — Thu 19 Nov (assignment)"
      },
      {
        "type": "text",
        "text": "{\n  \"now_local\": \"2026-11-16T23:38:22+09:00\",\n  \"timezone\": \"Asia/Tokyo\",\n  \"today\": \"2026-11-16\",\n  \"day_rollover\": \"02:00\",\n  \"overdue\": [\n    {\n      \"id\": \"pw-jdyu\",\n      \"title\": \"Late\",\n      \"kind\": \"other\",\n      \"due_at\": \"2020-01-01T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 30,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2020-01-02T02:00:00+09:00\",\n      \"due_day\": \"2020-01-01\",\n      \"due_label\": \"overdue (Wed 1 Jan)\",\n      \"overdue\": true,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    }\n  ],\n  \"due_today\": [],\n  \"next_7_days\": [\n    {\n      \"id\": \"pw-qgp9\",\n      \"title\": \"Thesis chapter\",\n      \"kind\": \"assignment\",\n      \"due_at\": \"2026-11-19T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 600,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-11-20T02:00:00+09:00\",\n      \"due_day\": \"2026-11-19\",\n      \"due_label\": \"Thu 19 Nov\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    }\n  ]\n}"
      }
    ],
    "structuredContent": {
      "now_local": "2026-11-16T23:38:22+09:00",
      "timezone": "Asia/Tokyo",
      "today": "2026-11-16",
      "day_rollover": "02:00",
      "overdue": [
        {
          "id": "pw-jdyu",
          "title": "Late",
          "kind": "other",
          "due_at": "2020-01-01T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 30,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2020-01-02T02:00:00+09:00",
          "due_day": "2020-01-01",
          "due_label": "overdue (Wed 1 Jan)",
          "overdue": true,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        }
      ],
      "due_today": [],
      "next_7_days": [
        {
          "id": "pw-qgp9",
          "title": "Thesis chapter",
          "kind": "assignment",
          "due_at": "2026-11-19T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 600,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-11-20T02:00:00+09:00",
          "due_day": "2026-11-19",
          "due_label": "Thu 19 Nov",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        }
      ]
    },
    "isError": false
  }
}
```

## 32. tools/call → plan_week

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 30,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 29
    },
    "name": "plan_week"
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 30,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Suggested sessions:\n- Mon 16 Nov 23:40–01:40 (Free time and admin): Residence card renewal [pw-fyaw]\n- Tue 17 Nov 01:40–01:55 (Free time and admin): Rent [pw-jppg]\n\nAt risk:\n- Late [pw-jdyu]: Already overdue.\n- Thesis chapter [pw-qgp9]: Needs 600 min in one block, but the most free time left in any block before the deadline (Thu 19 Nov) is 240 min (largest free block this week: 240 min)."
      },
      {
        "type": "text",
        "text": "{\n  \"generated_at\": \"2026-11-16T14:38:22.755973539Z\",\n  \"horizon_start\": \"2026-11-16T14:38:22.755973539Z\",\n  \"horizon_end\": \"2026-11-22T17:00:00Z\",\n  \"sessions\": [\n    {\n      \"item_id\": \"pw-fyaw\",\n      \"title\": \"Residence card renewal\",\n      \"start\": \"2026-11-16T14:40:00Z\",\n      \"end\": \"2026-11-16T16:40:00Z\",\n      \"start_local\": \"Mon 16 Nov 23:40\",\n      \"end_local\": \"Tue 17 Nov 01:40\",\n      \"block_label\": \"Free time and admin\",\n      \"day\": \"2026-11-16\",\n      \"due_at\": \"2026-11-26T01:00:00Z\",\n      \"minutes\": 120\n    },\n    {\n      \"item_id\": \"pw-jppg\",\n      \"title\": \"Rent\",\n      \"start\": \"2026-11-16T16:40:00Z\",\n      \"end\": \"2026-11-16T16:55:00Z\",\n      \"start_local\": \"Tue 17 Nov 01:40\",\n      \"end_local\": \"Tue 17 Nov 01:55\",\n      \"block_label\": \"Free time and admin\",\n      \"day\": \"2026-11-16\",\n      \"due_at\": \"2026-11-30T17:00:00Z\",\n      \"minutes\": 15\n    }\n  ],\n  \"at_risk\": [\n    {\n      \"item_id\": \"pw-jdyu\",\n      \"title\": \"Late\",\n      \"due_at\": \"2020-01-01T17:00:00Z\",\n      \"estimated_minutes\": 30,\n      \"reason\": \"Already overdue.\"\n    },\n    {\n      \"item_id\": \"pw-qgp9\",\n      \"title\": \"Thesis chapter\",\n      \"due_at\": \"2026-11-19T17:00:00Z\",\n      \"estimated_minutes\": 600,\n      \"reason\": \"Needs 600 min in one block, but the most free time left in any block before the deadline (Thu 19 Nov) is 240 min (largest free block this week: 240 min).\"\n    }\n  ],\n  \"later\": []\n}"
      }
    ],
    "structuredContent": {
      "generated_at": "2026-11-16T14:38:22.755973539Z",
      "horizon_start": "2026-11-16T14:38:22.755973539Z",
      "horizon_end": "2026-11-22T17:00:00Z",
      "sessions": [
        {
          "item_id": "pw-fyaw",
          "title": "Residence card renewal",
          "start": "2026-11-16T14:40:00Z",
          "end": "2026-11-16T16:40:00Z",
          "start_local": "Mon 16 Nov 23:40",
          "end_local": "Tue 17 Nov 01:40",
          "block_label": "Free time and admin",
          "day": "2026-11-16",
          "due_at": "2026-11-26T01:00:00Z",
          "minutes": 120
        },
        {
          "item_id": "pw-jppg",
          "title": "Rent",
          "start": "2026-11-16T16:40:00Z",
          "end": "2026-11-16T16:55:00Z",
          "start_local": "Tue 17 Nov 01:40",
          "end_local": "Tue 17 Nov 01:55",
          "block_label": "Free time and admin",
          "day": "2026-11-16",
          "due_at": "2026-11-30T17:00:00Z",
          "minutes": 15
        }
      ],
      "at_risk": [
        {
          "item_id": "pw-jdyu",
          "title": "Late",
          "due_at": "2020-01-01T17:00:00Z",
          "estimated_minutes": 30,
          "reason": "Already overdue."
        },
        {
          "item_id": "pw-qgp9",
          "title": "Thesis chapter",
          "due_at": "2026-11-19T17:00:00Z",
          "estimated_minutes": 600,
          "reason": "Needs 600 min in one block, but the most free time left in any block before the deadline (Thu 19 Nov) is 240 min (largest free block this week: 240 min)."
        }
      ],
      "later": []
    },
    "isError": false
  }
}
```

## 33. tools/call → delete_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 31,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 30
    },
    "name": "delete_item",
    "arguments": {
      "id": "pw-qgp9"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 31,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Deleted pw-qgp9 \"Thesis chapter\". It is in the archive and can be restored from the Paperweight app for 30 days."
      },
      {
        "type": "text",
        "text": "{\n  \"item\": {\n    \"id\": \"pw-qgp9\",\n    \"title\": \"Thesis chapter\",\n    \"kind\": \"assignment\",\n    \"due_at\": \"2026-11-19T17:00:00Z\",\n    \"all_day\": true,\n    \"status\": \"archived\",\n    \"notes\": \"\",\n    \"source\": null,\n    \"estimated_minutes\": 600,\n    \"snoozed_until\": null,\n    \"recurrence\": {\n      \"freq\": \"none\",\n      \"interval\": 1\n    },\n    \"steps\": [],\n    \"requirements\": [],\n    \"created_at\": \"2026-11-16T14:38:22Z\",\n    \"updated_at\": \"2026-11-16T14:38:22Z\",\n    \"completed_at\": null,\n    \"archived_at\": \"2026-11-16T14:38:22Z\",\n    \"due_local\": \"2026-11-20T02:00:00+09:00\",\n    \"due_day\": \"2026-11-19\",\n    \"due_label\": \"Thu 19 Nov\",\n    \"overdue\": false,\n    \"snoozed\": false,\n    \"snoozed_until_label\": null,\n    \"steps_done\": 0,\n    \"requirements_done\": 0\n  }\n}"
      }
    ],
    "structuredContent": {
      "item": {
        "id": "pw-qgp9",
        "title": "Thesis chapter",
        "kind": "assignment",
        "due_at": "2026-11-19T17:00:00Z",
        "all_day": true,
        "status": "archived",
        "notes": "",
        "source": null,
        "estimated_minutes": 600,
        "snoozed_until": null,
        "recurrence": {
          "freq": "none",
          "interval": 1
        },
        "steps": [],
        "requirements": [],
        "created_at": "2026-11-16T14:38:22Z",
        "updated_at": "2026-11-16T14:38:22Z",
        "completed_at": null,
        "archived_at": "2026-11-16T14:38:22Z",
        "due_local": "2026-11-20T02:00:00+09:00",
        "due_day": "2026-11-19",
        "due_label": "Thu 19 Nov",
        "overdue": false,
        "snoozed": false,
        "snoozed_until_label": null,
        "steps_done": 0,
        "requirements_done": 0
      }
    },
    "isError": false
  }
}
```

## 34. tools/call → update_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 32,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 31
    },
    "name": "update_item",
    "arguments": {
      "id": "pw-qgp9",
      "title": "z"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 32,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Item pw-qgp9 was deleted (it is in the archive and can be restored from the Paperweight app). Call list_items to find current items."
      }
    ],
    "isError": true
  }
}
```

## 35. tools/call → delete_item

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 33,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 32
    },
    "name": "delete_item",
    "arguments": {
      "id": "pw-qgp9"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 33,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Item pw-qgp9 was deleted (it is in the archive and can be restored from the Paperweight app). Call list_items to find current items."
      }
    ],
    "isError": true
  }
}
```

## 36. tools/call → list_items

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 34,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 33
    },
    "name": "list_items",
    "arguments": {
      "status": "all"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 34,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "4 items:\n- pw-jdyu \"Late\" — overdue (Wed 1 Jan) (other)\n- pw-fpep \"Rent\" — Sat 31 Jan (payment)\n- pw-fyaw \"Residence card renewal\" — Thu 26 Nov 10:00 (paperwork)\n- pw-jppg \"Rent\" — Mon 30 Nov (payment)"
      },
      {
        "type": "text",
        "text": "{\n  \"count\": 4,\n  \"items\": [\n    {\n      \"id\": \"pw-jdyu\",\n      \"title\": \"Late\",\n      \"kind\": \"other\",\n      \"due_at\": \"2020-01-01T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 30,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2020-01-02T02:00:00+09:00\",\n      \"due_day\": \"2020-01-01\",\n      \"due_label\": \"overdue (Wed 1 Jan)\",\n      \"overdue\": true,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    },\n    {\n      \"id\": \"pw-fpep\",\n      \"title\": \"Rent\",\n      \"kind\": \"payment\",\n      \"due_at\": \"2026-01-31T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"done\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 15,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"monthly\",\n        \"interval\": 1\n      },\n      \"recurrence_anchor_day\": 31,\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": \"2026-11-16T14:38:22Z\",\n      \"archived_at\": null,\n      \"due_local\": \"2026-02-01T02:00:00+09:00\",\n      \"due_day\": \"2026-01-31\",\n      \"due_label\": \"Sat 31 Jan\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    },\n    {\n      \"id\": \"pw-fyaw\",\n      \"title\": \"Residence card renewal\",\n      \"kind\": \"paperwork\",\n      \"due_at\": \"2026-11-26T01:00:00Z\",\n      \"all_day\": false,\n      \"status\": \"open\",\n      \"notes\": \"Immigration office, morning\",\n      \"source\": null,\n      \"estimated_minutes\": 120,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [\n        {\n          \"id\": 1,\n          \"title\": \"Get photo\",\n          \"done\": false,\n          \"soft_due_at\": \"2026-11-18T17:00:00Z\",\n          \"position\": 0\n        },\n        {\n          \"id\": 2,\n          \"title\": \"Fill in form\",\n          \"done\": false,\n          \"soft_due_at\": null,\n          \"position\": 1\n        },\n        {\n          \"id\": 3,\n          \"title\": \"Book appointment\",\n          \"done\": false,\n          \"soft_due_at\": null,\n          \"position\": 2\n        }\n      ],\n      \"requirements\": [\n        {\n          \"id\": 1,\n          \"text\": \"Passport\",\n          \"done\": true,\n          \"position\": 0\n        },\n        {\n          \"id\": 2,\n          \"text\": \"Current card\",\n          \"done\": false,\n          \"position\": 1\n        },\n        {\n          \"id\": 4,\n          \"text\": \"Revenue stamp\",\n          \"done\": false,\n          \"position\": 2\n        }\n      ],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-11-26T10:00:00+09:00\",\n      \"due_day\": \"2026-11-26\",\n      \"due_label\": \"Thu 26 Nov 10:00\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 1\n    },\n    {\n      \"id\": \"pw-jppg\",\n      \"title\": \"Rent\",\n      \"kind\": \"payment\",\n      \"due_at\": \"2026-11-30T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 15,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"monthly\",\n        \"interval\": 1\n      },\n      \"recurrence_anchor_day\": 31,\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2026-12-01T02:00:00+09:00\",\n      \"due_day\": \"2026-11-30\",\n      \"due_label\": \"Mon 30 Nov\",\n      \"overdue\": false,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    }\n  ]\n}"
      }
    ],
    "structuredContent": {
      "count": 4,
      "items": [
        {
          "id": "pw-jdyu",
          "title": "Late",
          "kind": "other",
          "due_at": "2020-01-01T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 30,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2020-01-02T02:00:00+09:00",
          "due_day": "2020-01-01",
          "due_label": "overdue (Wed 1 Jan)",
          "overdue": true,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        },
        {
          "id": "pw-fpep",
          "title": "Rent",
          "kind": "payment",
          "due_at": "2026-01-31T17:00:00Z",
          "all_day": true,
          "status": "done",
          "notes": "",
          "source": null,
          "estimated_minutes": 15,
          "snoozed_until": null,
          "recurrence": {
            "freq": "monthly",
            "interval": 1
          },
          "recurrence_anchor_day": 31,
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": "2026-11-16T14:38:22Z",
          "archived_at": null,
          "due_local": "2026-02-01T02:00:00+09:00",
          "due_day": "2026-01-31",
          "due_label": "Sat 31 Jan",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        },
        {
          "id": "pw-fyaw",
          "title": "Residence card renewal",
          "kind": "paperwork",
          "due_at": "2026-11-26T01:00:00Z",
          "all_day": false,
          "status": "open",
          "notes": "Immigration office, morning",
          "source": null,
          "estimated_minutes": 120,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [
            {
              "id": 1,
              "title": "Get photo",
              "done": false,
              "soft_due_at": "2026-11-18T17:00:00Z",
              "position": 0
            },
            {
              "id": 2,
              "title": "Fill in form",
              "done": false,
              "soft_due_at": null,
              "position": 1
            },
            {
              "id": 3,
              "title": "Book appointment",
              "done": false,
              "soft_due_at": null,
              "position": 2
            }
          ],
          "requirements": [
            {
              "id": 1,
              "text": "Passport",
              "done": true,
              "position": 0
            },
            {
              "id": 2,
              "text": "Current card",
              "done": false,
              "position": 1
            },
            {
              "id": 4,
              "text": "Revenue stamp",
              "done": false,
              "position": 2
            }
          ],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-11-26T10:00:00+09:00",
          "due_day": "2026-11-26",
          "due_label": "Thu 26 Nov 10:00",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 1
        },
        {
          "id": "pw-jppg",
          "title": "Rent",
          "kind": "payment",
          "due_at": "2026-11-30T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 15,
          "snoozed_until": null,
          "recurrence": {
            "freq": "monthly",
            "interval": 1
          },
          "recurrence_anchor_day": 31,
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2026-12-01T02:00:00+09:00",
          "due_day": "2026-11-30",
          "due_label": "Mon 30 Nov",
          "overdue": false,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        }
      ]
    },
    "isError": false
  }
}
```

## 37. resources/list

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 35,
  "method": "resources/list",
  "params": {
    "_meta": {
      "progressToken": 34
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 35,
  "result": {
    "resources": [
      {
        "uri": "paperweight://routine",
        "name": "routine",
        "title": "Weekly routine and settings",
        "description": "The user's weekly routine blocks (which ones are free for tasks), timezone, day rollover, quiet hours and reminder offsets. Read this to understand when the user can work and how 'today' is defined.",
        "mimeType": "application/json"
      }
    ]
  }
}
```

## 38. resources/read

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 36,
  "method": "resources/read",
  "params": {
    "_meta": {
      "progressToken": 35
    },
    "uri": "paperweight://routine"
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 36,
  "result": {
    "contents": [
      {
        "uri": "paperweight://routine",
        "mimeType": "application/json",
        "text": "{\n  \"now_local\": \"2026-11-16T23:38:22+09:00\",\n  \"timezone\": \"Asia/Tokyo\",\n  \"today\": \"2026-11-16\",\n  \"weekday_numbering\": \"0 = Monday … 6 = Sunday; a block's weekday is the logical day, so a 22:00–02:00 block runs into the next calendar morning\",\n  \"routine\": [\n    {\n      \"id\": 1,\n      \"weekday\": 0,\n      \"start\": \"12:00\",\n      \"end\": \"14:00\",\n      \"label\": \"Wake up, eat, get going\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 8,\n      \"weekday\": 0,\n      \"start\": \"14:00\",\n      \"end\": \"18:00\",\n      \"label\": \"Assignments and research\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 15,\n      \"weekday\": 0,\n      \"start\": \"18:00\",\n      \"end\": \"21:00\",\n      \"label\": \"Cooking and chores\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 22,\n      \"weekday\": 0,\n      \"start\": \"22:00\",\n      \"end\": \"02:00\",\n      \"label\": \"Free time and admin\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 2,\n      \"weekday\": 1,\n      \"start\": \"12:00\",\n      \"end\": \"14:00\",\n      \"label\": \"Wake up, eat, get going\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 9,\n      \"weekday\": 1,\n      \"start\": \"14:00\",\n      \"end\": \"18:00\",\n      \"label\": \"Assignments and research\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 16,\n      \"weekday\": 1,\n      \"start\": \"18:00\",\n      \"end\": \"21:00\",\n      \"label\": \"Cooking and chores\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 23,\n      \"weekday\": 1,\n      \"start\": \"22:00\",\n      \"end\": \"02:00\",\n      \"label\": \"Free time and admin\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 3,\n      \"weekday\": 2,\n      \"start\": \"12:00\",\n      \"end\": \"14:00\",\n      \"label\": \"Wake up, eat, get going\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 10,\n      \"weekday\": 2,\n      \"start\": \"14:00\",\n      \"end\": \"18:00\",\n      \"label\": \"Assignments and research\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 17,\n      \"weekday\": 2,\n      \"start\": \"18:00\",\n      \"end\": \"21:00\",\n      \"label\": \"Cooking and chores\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 24,\n      \"weekday\": 2,\n      \"start\": \"22:00\",\n      \"end\": \"02:00\",\n      \"label\": \"Free time and admin\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 4,\n      \"weekday\": 3,\n      \"start\": \"12:00\",\n      \"end\": \"14:00\",\n      \"label\": \"Wake up, eat, get going\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 11,\n      \"weekday\": 3,\n      \"start\": \"14:00\",\n      \"end\": \"18:00\",\n      \"label\": \"Assignments and research\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 18,\n      \"weekday\": 3,\n      \"start\": \"18:00\",\n      \"end\": \"21:00\",\n      \"label\": \"Cooking and chores\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 25,\n      \"weekday\": 3,\n      \"start\": \"22:00\",\n      \"end\": \"02:00\",\n      \"label\": \"Free time and admin\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 5,\n      \"weekday\": 4,\n      \"start\": \"12:00\",\n      \"end\": \"14:00\",\n      \"label\": \"Wake up, eat, get going\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 12,\n      \"weekday\": 4,\n      \"start\": \"14:00\",\n      \"end\": \"18:00\",\n      \"label\": \"Assignments and research\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 19,\n      \"weekday\": 4,\n      \"start\": \"18:00\",\n      \"end\": \"21:00\",\n      \"label\": \"Cooking and chores\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 26,\n      \"weekday\": 4,\n      \"start\": \"22:00\",\n      \"end\": \"02:00\",\n      \"label\": \"Free time and admin\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 6,\n      \"weekday\": 5,\n      \"start\": \"12:00\",\n      \"end\": \"14:00\",\n      \"label\": \"Wake up, eat, get going\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 13,\n      \"weekday\": 5,\n      \"start\": \"14:00\",\n      \"end\": \"18:00\",\n      \"label\": \"Assignments and research\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 20,\n      \"weekday\": 5,\n      \"start\": \"18:00\",\n      \"end\": \"21:00\",\n      \"label\": \"Cooking and chores\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 27,\n      \"weekday\": 5,\n      \"start\": \"22:00\",\n      \"end\": \"02:00\",\n      \"label\": \"Free time and admin\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 7,\n      \"weekday\": 6,\n      \"start\": \"12:00\",\n      \"end\": \"14:00\",\n      \"label\": \"Wake up, eat, get going\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 14,\n      \"weekday\": 6,\n      \"start\": \"14:00\",\n      \"end\": \"18:00\",\n      \"label\": \"Assignments and research\",\n      \"available_for_tasks\": true\n    },\n    {\n      \"id\": 21,\n      \"weekday\": 6,\n      \"start\": \"18:00\",\n      \"end\": \"21:00\",\n      \"label\": \"Cooking and chores\",\n      \"available_for_tasks\": false\n    },\n    {\n      \"id\": 28,\n      \"weekday\": 6,\n      \"start\": \"22:00\",\n      \"end\": \"02:00\",\n      \"label\": \"Free time and admin\",\n      \"available_for_tasks\": true\n    }\n  ],\n  \"settings\": {\n    \"timezone\": \"system\",\n    \"day_rollover\": \"02:00\",\n    \"quiet_start\": \"02:00\",\n    \"quiet_end\": \"12:00\",\n    \"reminder_offsets\": [\n      \"7d\",\n      \"2d\",\n      \"day\",\n      \"overdue\"\n    ],\n    \"feed_port\": 47800,\n    \"language\": \"en\",\n    \"quick_add_shortcut\": \"CommandOrControl+Shift+Space\"\n  }\n}"
      }
    ]
  }
}
```

## 39. resources/read

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 37,
  "method": "resources/read",
  "params": {
    "_meta": {
      "progressToken": 36
    },
    "uri": "paperweight://nope"
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 37,
  "error": {
    "code": -32002,
    "message": "No resource paperweight://nope. The only resource is paperweight://routine."
  }
}
```

## 40. prompts/list

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 38,
  "method": "prompts/list",
  "params": {
    "_meta": {
      "progressToken": 37
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 38,
  "result": {
    "prompts": [
      {
        "name": "weekly_review",
        "description": "Walk through the coming week with the user: clear overdue items, check what each deadline needs, and agree when to do the work.",
        "arguments": [
          {
            "name": "focus",
            "description": "Optional focus for the review, e.g. \"paperwork only\".",
            "required": false
          }
        ]
      }
    ]
  }
}
```

## 41. prompts/get

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 39,
  "method": "prompts/get",
  "params": {
    "_meta": {
      "progressToken": 38
    },
    "name": "weekly_review"
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 39,
  "result": {
    "description": "Weekly review of Paperweight items",
    "messages": [
      {
        "role": "user",
        "content": {
          "type": "text",
          "text": "Let's do my weekly review in Paperweight. Keep it short and calm — I find long planning sessions hard, so one thing at a time.\n\n1. Call get_brief and plan_week.\n2. Overdue first. For each overdue item, ask me one question: is it done (complete_item), still needed with a new date (update_item), or no longer needed (delete_item)?\n3. Then this week's deadlines, earliest first. For each one, read me what's still missing from its requirements and steps, and tick off anything I say is ready (update_item).\n4. Anything plan_week marks as at risk: tell me why and suggest one concrete fix (start it today, split it into steps, or move a deadline if that's really possible).\n5. Ask if there's anything new I've been meaning to track — paperwork, payments, emails I need to answer — and add it with add_item.\n6. Finish with a three-line summary: what's done, what's next, and the one thing to do tomorrow."
        }
      }
    ]
  }
}
```

## 42. tools/call → get_brief

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 40,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": 39
    },
    "name": "get_brief"
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 40,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Now: 2026-11-16T23:38:22+09:00 (Asia/Tokyo). Today is 2026-11-16 (the day ends at 02:00).\nOverdue:\n- pw-jdyu \"Late\" — overdue (Wed 1 Jan) (other)\nDue today: nothing.\nNext 7 days: nothing."
      },
      {
        "type": "text",
        "text": "{\n  \"now_local\": \"2026-11-16T23:38:22+09:00\",\n  \"timezone\": \"Asia/Tokyo\",\n  \"today\": \"2026-11-16\",\n  \"day_rollover\": \"02:00\",\n  \"overdue\": [\n    {\n      \"id\": \"pw-jdyu\",\n      \"title\": \"Late\",\n      \"kind\": \"other\",\n      \"due_at\": \"2020-01-01T17:00:00Z\",\n      \"all_day\": true,\n      \"status\": \"open\",\n      \"notes\": \"\",\n      \"source\": null,\n      \"estimated_minutes\": 30,\n      \"snoozed_until\": null,\n      \"recurrence\": {\n        \"freq\": \"none\",\n        \"interval\": 1\n      },\n      \"steps\": [],\n      \"requirements\": [],\n      \"created_at\": \"2026-11-16T14:38:22Z\",\n      \"updated_at\": \"2026-11-16T14:38:22Z\",\n      \"completed_at\": null,\n      \"archived_at\": null,\n      \"due_local\": \"2020-01-02T02:00:00+09:00\",\n      \"due_day\": \"2020-01-01\",\n      \"due_label\": \"overdue (Wed 1 Jan)\",\n      \"overdue\": true,\n      \"snoozed\": false,\n      \"snoozed_until_label\": null,\n      \"steps_done\": 0,\n      \"requirements_done\": 0\n    }\n  ],\n  \"due_today\": [],\n  \"next_7_days\": []\n}"
      }
    ],
    "structuredContent": {
      "now_local": "2026-11-16T23:38:22+09:00",
      "timezone": "Asia/Tokyo",
      "today": "2026-11-16",
      "day_rollover": "02:00",
      "overdue": [
        {
          "id": "pw-jdyu",
          "title": "Late",
          "kind": "other",
          "due_at": "2020-01-01T17:00:00Z",
          "all_day": true,
          "status": "open",
          "notes": "",
          "source": null,
          "estimated_minutes": 30,
          "snoozed_until": null,
          "recurrence": {
            "freq": "none",
            "interval": 1
          },
          "steps": [],
          "requirements": [],
          "created_at": "2026-11-16T14:38:22Z",
          "updated_at": "2026-11-16T14:38:22Z",
          "completed_at": null,
          "archived_at": null,
          "due_local": "2020-01-02T02:00:00+09:00",
          "due_day": "2020-01-01",
          "due_label": "overdue (Wed 1 Jan)",
          "overdue": true,
          "snoozed": false,
          "snoozed_until_label": null,
          "steps_done": 0,
          "requirements_done": 0
        }
      ],
      "due_today": [],
      "next_7_days": []
    },
    "isError": false
  }
}
```

