//! Tray icon: the count of overdue and due-today items, and a menu with the
//! next five items plus Open, Quick add and Quit.

use chrono::Utc;
use paperweight_core::brief::{ItemView, brief, sort_by_priority};
use tauri::image::Image;
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::AppState;

const TRAY_ID: &str = "paperweight";

struct Snapshot {
    urgent: usize,
    overdue: usize,
    today: usize,
    next: Vec<ItemView>,
}

fn snapshot(app: &AppHandle) -> Option<Snapshot> {
    let state = app.state::<AppState>();
    let store = state.store().ok()?;
    let clock = store.clock().ok()?;
    let now = Utc::now();
    let items = store.open_items().ok()?;
    let b = brief(items.clone(), &clock, now);
    let mut all: Vec<ItemView> = items
        .into_iter()
        .map(|i| ItemView::new(i, &clock, now))
        .collect();
    sort_by_priority(&mut all);
    all.truncate(5);
    Some(Snapshot {
        urgent: b.overdue.len() + b.due_today.len(),
        overdue: b.overdue.len(),
        today: b.due_today.len(),
        next: all,
    })
}

fn menu(app: &AppHandle, snap: Option<&Snapshot>) -> tauri::Result<Menu<Wry>> {
    let mut m = MenuBuilder::new(app);
    match snap {
        Some(s) if !s.next.is_empty() => {
            for v in &s.next {
                let mut title = v.item.title.clone();
                if title.chars().count() > 40 {
                    title = title.chars().take(39).collect::<String>() + "…";
                }
                let label = format!("{} — {}", title, v.due_label);
                m = m.item(
                    &MenuItemBuilder::with_id(format!("item:{}", v.item.id), label).build(app)?,
                );
            }
        }
        _ => {
            m = m.item(
                &MenuItemBuilder::with_id("none", "Nothing coming up")
                    .enabled(false)
                    .build(app)?,
            );
        }
    }
    if let Some(v) = crate::updates::pending_version(app) {
        m = m
            .separator()
            .item(&MenuItemBuilder::with_id("update", format!("Update to {v}…")).build(app)?);
    }
    m.separator()
        .item(&MenuItemBuilder::with_id("open", "Open Paperweight").build(app)?)
        .item(&MenuItemBuilder::with_id("quick", "Quick add…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit Paperweight").build(app)?)
        .build()
}

fn icon(count: usize) -> Image<'static> {
    let (rgba, w, h) = crate::badge::render(count);
    Image::new_owned(rgba, w, h)
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let snap = snapshot(app);
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(snap.as_ref().map(|s| s.urgent).unwrap_or(0)))
        .tooltip("Paperweight")
        .menu(&menu(app, snap.as_ref())?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "open" => crate::show_main(app),
                "quick" => crate::open_quick_add(app),
                "update" => crate::open_view(app, "settings"),
                "quit" => app.exit(0),
                other => {
                    if let Some(item) = other.strip_prefix("item:") {
                        crate::open_item(app, item);
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::show_main(tray.app_handle());
            }
        })
        .build(app)?;
    refresh(app);
    Ok(())
}

/// Recompute the count and the menu. Called when data changes and once a minute.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let snap = snapshot(app);
    let urgent = snap.as_ref().map(|s| s.urgent).unwrap_or(0);
    let _ = tray.set_icon(Some(icon(urgent)));
    let tip = match &snap {
        Some(s) if s.urgent > 0 => {
            format!("Paperweight — {} overdue, {} due today", s.overdue, s.today)
        }
        _ => "Paperweight — nothing due today".to_string(),
    };
    let _ = tray.set_tooltip(Some(&tip));
    // A text count next to the icon where the platform supports it (macOS, Linux).
    #[cfg(not(target_os = "windows"))]
    let _ = tray.set_title(if urgent > 0 {
        Some(urgent.to_string())
    } else {
        None
    });
    if let Ok(m) = menu(app, snap.as_ref()) {
        let _ = tray.set_menu(Some(m));
    }
}
