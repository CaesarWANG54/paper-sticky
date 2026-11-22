//! The commands the three pages call.
//!
//! Commands that create, destroy or restyle windows are `async`: a synchronous
//! command runs on the main thread, and on Windows a webview cannot finish
//! initialising while that thread is blocked inside the command.

use tauri::{AppHandle, Manager, State};

use crate::store::{now_ms, valid_color, Data, Note, PATTERNS, SHAPES};
use crate::{emit_changed, hover, windows, AppState};

#[tauri::command]
pub fn list_notes(state: State<AppState>) -> Data {
    state.store.lock().unwrap().snapshot()
}

#[tauri::command]
pub fn get_note(state: State<AppState>, id: String) -> Option<Note> {
    state.store.lock().unwrap().note(&id).cloned()
}

#[tauri::command]
pub async fn create_note(app: AppHandle, state: State<'_, AppState>) -> Result<Note, String> {
    let note = state.store.lock().unwrap().create_note();
    hover::force_hide(&app);
    let rect = windows::open_note_window(&app, &note).map_err(|e| e.to_string())?;
    windows::remember_rect(&app, &note.id, rect);
    emit_changed(&app);
    Ok(note)
}

#[tauri::command]
pub async fn update_note(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    content: Option<String>,
    color: Option<String>,
    shape: Option<String>,
    pattern: Option<String>,
) -> Result<(), String> {
    if let Some(c) = &color {
        if !valid_color(c) {
            return Err(format!("unknown colour {c}"));
        }
    }
    if let Some(p) = &pattern {
        if !PATTERNS.contains(&p.as_str()) {
            return Err("unknown paper pattern".into());
        }
    }
    if let Some(sh) = &shape {
        if !SHAPES.contains(&sh.as_str()) {
            return Err(format!("unknown shape {sh}"));
        }
    }
    {
        let mut store = state.store.lock().unwrap();
        let note = store.note_mut(&id).ok_or("no such note")?;
        if let Some(content) = content {
            note.content = content;
        }
        if let Some(color) = color {
            note.color = color;
        }
        if let Some(shape) = shape {
            note.shape = shape;
        }
        if let Some(pattern) = pattern {
            note.pattern = pattern;
        }
        note.updated_at = now_ms();
        store.save_if_dirty().map_err(|e| e.to_string())?;
    }
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn delete_note(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    if !state.store.lock().unwrap().delete_note(&id) {
        return Err("no such note".into());
    }
    windows::close_note_window(&app, &id);
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn open_note(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let note = {
        let mut store = state.store.lock().unwrap();
        let note = store.note_mut(&id).ok_or("no such note")?;
        note.open = true;
        note.clone()
    };
    hover::force_hide(&app);
    let rect = windows::open_note_window(&app, &note).map_err(|e| e.to_string())?;
    windows::remember_rect(&app, &note.id, rect);
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn close_note(app: AppHandle, id: String) -> Result<(), String> {
    mark_closed(&app, &id);
    windows::set_tab_hidden(&app, false);
    crate::flush_now(&app);
    windows::close_note_window(&app, &id);
    Ok(())
}

/// Native dragging can return before the mouse is released on some WebView2 versions.
#[tauri::command]
pub async fn finish_drag(app: AppHandle, id: String) -> Result<bool, String> {
    for _ in 0..1200 {
        if !windows::left_mouse_down() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    if windows::left_mouse_down() {
        return Ok(false);
    }
    let Some(window) = app.get_webview_window(&windows::note_label(&id)) else {
        return Ok(false);
    };
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let area = windows::work_area(&app);
    let right = (pos.x as f64 + size.width as f64) / scale - crate::geometry::NOTE_MARGIN;
    let y = pos.y as f64 / scale;
    if right < area.x + area.w - 26.0 || y > area.y + area.h || y + size.height as f64 / scale < area.y {
        return Ok(false);
    }
    {
        let state = app.state::<AppState>();
        let mut store = state.store.lock().unwrap();
        if let Some(note) = store.note_mut(&id) {
            if let Some(rect) = note.window.as_mut() {
                rect.x = (area.x + area.w - rect.w - 58.0).max(area.x + 16.0);
            }
        }
    }
    close_note(app, id).await?;
    Ok(true)
}

/// Records that a note's window is gone. Safe to call more than once.
pub fn mark_closed(app: &AppHandle, id: &str) {
    let changed = {
        let state = app.state::<AppState>();
        let mut store = state.store.lock().unwrap();
        match store.note_mut(id) {
            Some(note) if note.open => {
                note.open = false;
                true
            }
            _ => false,
        }
    };
    if changed {
        emit_changed(app);
    }
}

#[tauri::command]
pub async fn set_note_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> Result<(), String> {
    {
        let mut store = state.store.lock().unwrap();
        store.note_mut(&id).ok_or("no such note")?.pinned = pinned;
    }
    windows::set_note_on_top(&app, &id, pinned);
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn hover(app: AppHandle, source: String, inside: bool) {
    hover::hover(&app, &source, inside);
}

#[tauri::command]
pub fn pile_hidden(app: AppHandle) {
    hover::pile_hidden(&app);
}

#[tauri::command]
pub fn tab_drag(app: AppHandle, top: f64, done: bool) {
    windows::tab_drag(&app, top, done);
}
