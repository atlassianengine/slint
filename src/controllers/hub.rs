// Rust guideline compliant 2026-02-21

//! Daily Hub controller backing tasks checklist, habits tracker, and quick notes.

use crate::AppWindow;
use crate::{HabitItem, HubTaskItem};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Installs the Daily Hub controller onto the main Slint `AppWindow`.
pub fn install(app: &AppWindow) {
    let habits_model = Rc::new(VecModel::from(vec![
        HabitItem {
            id: "habit-1".into(),
            name: "Write specifications".into(),
            completed: true,
        },
        HabitItem {
            id: "habit-2".into(),
            name: "Verify unit tests".into(),
            completed: false,
        },
        HabitItem {
            id: "habit-3".into(),
            name: "Refactor Rust structures".into(),
            completed: false,
        },
    ]));

    let tasks_model = Rc::new(VecModel::from(vec![
        HubTaskItem {
            id: "task-1".into(),
            title: "Open and wire document editor nodes on canvas".into(),
            completed: false,
        },
        HubTaskItem {
            id: "task-2".into(),
            title: "Build and verify daily hub views".into(),
            completed: true,
        },
    ]));

    app.set_hub_habits(ModelRc::from(habits_model.clone()));
    app.set_hub_tasks(ModelRc::from(tasks_model.clone()));
    app.set_hub_quick_note_text("This is an initial workspace quick note entry.".into());
    app.set_hub_day_progress(0.5);

    // Wire callbacks
    {
        let habits_model_c = habits_model.clone();
        let tasks_model_c = tasks_model.clone();
        let app_weak = app.as_weak();
        app.on_hub_habit_toggled(move |id, val| {
            for idx in 0..habits_model_c.row_count() {
                if let Some(mut habit) = habits_model_c.row_data(idx) {
                    if habit.id == id {
                        habit.completed = val;
                        habits_model_c.set_row_data(idx, habit);
                        break;
                    }
                }
            }
            if let Some(app) = app_weak.upgrade() {
                update_progress(&app, &habits_model_c, &tasks_model_c);
            }
        });
    }

    {
        let habits_model_c = habits_model.clone();
        let tasks_model_c = tasks_model.clone();
        let app_weak = app.as_weak();
        app.on_hub_task_toggled(move |id, val| {
            for idx in 0..tasks_model_c.row_count() {
                if let Some(mut task) = tasks_model_c.row_data(idx) {
                    if task.id == id {
                        task.completed = val;
                        tasks_model_c.set_row_data(idx, task);
                        break;
                    }
                }
            }
            if let Some(app) = app_weak.upgrade() {
                update_progress(&app, &habits_model_c, &tasks_model_c);
            }
        });
    }

    app.on_hub_save_quick_note(move |text| {
        println!("Daily Hub note saved: {}", text);
    });

    {
        let app_weak = app.as_weak();
        app.on_hub_open_today_journal(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_active_surface(crate::WorkbenchSurface::Document);
            }
        });
    }
}

fn update_progress(app: &AppWindow, habits: &VecModel<HabitItem>, tasks: &VecModel<HubTaskItem>) {
    let mut total = 0;
    let mut completed = 0;

    for idx in 0..habits.row_count() {
        total += 1;
        if habits.row_data(idx).is_some_and(|h| h.completed) {
            completed += 1;
        }
    }

    for idx in 0..tasks.row_count() {
        total += 1;
        if tasks.row_data(idx).is_some_and(|t| t.completed) {
            completed += 1;
        }
    }

    if total > 0 {
        app.set_hub_day_progress(completed as f32 / total as f32);
    }
}
