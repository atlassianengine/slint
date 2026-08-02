// Rust guideline compliant 2026-02-21

//! Reactive callbacks connecting Slint UI events to `PlannerSession`.

use chrono::{Datelike, NaiveDate};
use planner_core::{
    format_day_key, parse_day_key, week_days_for_anchor, PlannerItem, PlannerSession,
    PlannerTimeZone, PlannerViewMode,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

use crate::planner::presentation::{
    daily_columns, items_to_slint_model, month_cells_to_slint_model, view_mode_from_slint,
    view_mode_to_slint, week_day_items, year_cells_to_slint_model, year_summaries_to_slint_model,
};
use crate::{AppWindow, PlannerViewMode as SlintViewMode};

/// Installs the planner feature onto the main Slint `AppWindow`.
pub fn install(app: &AppWindow) {
    let today = NaiveDate::from_ymd_opt(2026, 7, 20).unwrap();
    let mut session = PlannerSession::new(today, today, PlannerTimeZone::Utc);

    // Populate initial demo planner items.
    session.items = vec![
        PlannerItem {
            id: "task-1".to_string(),
            title: "Architecture Review".to_string(),
            subtitle: "WGPU Slint Migration".to_string(),
            day_key: "2026-07-20".to_string(),
            start_time_mins: 540, // 09:00
            duration_mins: 90,
            accent_hex: "#38bdfc".to_string(),
            completed: false,
            selected: false,
        },
        PlannerItem {
            id: "task-2".to_string(),
            title: "Rostock OVG Audit".to_string(),
            subtitle: "Pathfinding Verification".to_string(),
            day_key: "2026-07-20".to_string(),
            start_time_mins: 690, // 11:30
            duration_mins: 60,
            accent_hex: "#a855f7".to_string(),
            completed: true,
            selected: false,
        },
        PlannerItem {
            id: "task-3".to_string(),
            title: "Design Studio Sync".to_string(),
            subtitle: "Planner & Timeline Surfaces".to_string(),
            day_key: "2026-07-21".to_string(),
            start_time_mins: 840, // 14:00
            duration_mins: 120,
            accent_hex: "#22c55e".to_string(),
            completed: false,
            selected: false,
        },
    ];

    let session = Rc::new(RefCell::new(session));
    sync_session_to_ui(app, &session.borrow());

    // View Mode Switcher callback
    let session_clone = Rc::clone(&session);
    app.on_planner_set_view_mode({
        let weak_app = app.as_weak();
        move |mode: SlintViewMode| {
            let mut s = session_clone.borrow_mut();
            s.set_view_mode(view_mode_from_slint(mode));
            if let Some(app) = weak_app.upgrade() {
                sync_session_to_ui(&app, &s);
            }
        }
    });

    // Navigate Prev callback
    let session_clone = Rc::clone(&session);
    app.on_planner_navigate_prev({
        let weak_app = app.as_weak();
        move || {
            let mut s = session_clone.borrow_mut();
            s.navigate_prev();
            if let Some(app) = weak_app.upgrade() {
                sync_session_to_ui(&app, &s);
            }
        }
    });

    // Navigate Next callback
    let session_clone = Rc::clone(&session);
    app.on_planner_navigate_next({
        let weak_app = app.as_weak();
        move || {
            let mut s = session_clone.borrow_mut();
            s.navigate_next();
            if let Some(app) = weak_app.upgrade() {
                sync_session_to_ui(&app, &s);
            }
        }
    });

    // Navigate Today callback
    let session_clone = Rc::clone(&session);
    app.on_planner_navigate_today({
        let weak_app = app.as_weak();
        move || {
            let mut s = session_clone.borrow_mut();
            s.navigate_today();
            if let Some(app) = weak_app.upgrade() {
                sync_session_to_ui(&app, &s);
            }
        }
    });

    // Open day from weekly/monthly interactions.
    let session_clone = Rc::clone(&session);
    app.on_planner_open_day({
        let weak_app = app.as_weak();
        move |day_key| {
            if let Some(day) = parse_day_key(day_key.as_str()) {
                let mut s = session_clone.borrow_mut();
                s.anchor_date = day;
                s.view_mode = PlannerViewMode::Daily;
                if let Some(app) = weak_app.upgrade() {
                    sync_session_to_ui(&app, &s);
                }
            }
        }
    });

    // Open month from yearly calendar.
    let session_clone = Rc::clone(&session);
    app.on_planner_open_month({
        let weak_app = app.as_weak();
        move |month| {
            if !(1..=12).contains(&month) {
                return;
            }
            let month_u32 = month as u32;
            let mut s = session_clone.borrow_mut();
            if let Some(next_anchor) = NaiveDate::from_ymd_opt(s.anchor_date.year(), month_u32, 1) {
                s.anchor_date = next_anchor;
                s.view_mode = PlannerViewMode::Monthly;
            }
            if let Some(app) = weak_app.upgrade() {
                sync_session_to_ui(&app, &s);
            }
        }
    });

    // Item Selection callback
    let session_clone = Rc::clone(&session);
    app.on_planner_item_selected({
        let weak_app = app.as_weak();
        move |id| {
            let mut s = session_clone.borrow_mut();
            for item in &mut s.items {
                item.selected = item.id.as_str() == id.as_str();
            }
            if let Some(app) = weak_app.upgrade() {
                sync_session_to_ui(&app, &s);
            }
        }
    });
}

fn sync_session_to_ui(app: &AppWindow, session: &PlannerSession) {
    app.set_planner_view_mode(view_mode_to_slint(session.view_mode));
    app.set_planner_header_title(session.title_header().into());
    app.set_planner_active_day_key(format_day_key(session.anchor_date).into());

    let week_keys: Vec<slint::SharedString> = week_days_for_anchor(session.anchor_date)
        .into_iter()
        .map(|d| format_day_key(d).into())
        .collect();
    let week_key_strings: Vec<String> = week_keys
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    app.set_planner_week_day_keys(ModelRc::new(VecModel::from(week_keys)));
    app.set_planner_week_day_columns(ModelRc::new(VecModel::from(week_day_items(
        &week_key_strings,
        &session.items,
    ))));
    app.set_planner_daily_columns(ModelRc::new(VecModel::from(daily_columns(
        &session.items,
        &format_day_key(session.anchor_date),
    ))));

    app.set_planner_items(items_to_slint_model(&session.items));
    app.set_planner_month_cells(month_cells_to_slint_model(
        &session.current_month_cells(),
        &session.items,
    ));
    app.set_planner_year_cells(ModelRc::new(VecModel::from(year_cells_to_slint_model(
        session.anchor_date.year(),
        session.today_date,
        &session.items,
    ))));
    app.set_planner_year_summaries(year_summaries_to_slint_model(
        &session.current_year_summaries(),
    ));
}
