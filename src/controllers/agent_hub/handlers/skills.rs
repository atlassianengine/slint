// Rust guideline compliant 2026-02-21

use std::cell::RefCell;
use std::rc::Rc;

use crate::{AppWindow, SkillItem};
use slint::{ComponentHandle, Model, VecModel};

use super::super::helpers::{append_log, logs_to_string};

pub(crate) fn register_skill_selected(
    app: &AppWindow,
    skills: Rc<VecModel<SkillItem>>,
    logs: Rc<RefCell<Vec<String>>>,
    selected_skill_id: Rc<RefCell<String>>,
) {
    let app_weak = app.as_weak();
    app.on_orchestrator_skill_selected(move |skill_id| {
        let skill_id = skill_id.to_string();
        let mut instructions = "No instructions available".to_string();
        for idx in 0..skills.row_count() {
            if let Some(skill) = skills.row_data(idx) {
                if skill.id == skill_id {
                    instructions = format!(
                        "SKILL.md instructions for {}:\n{}",
                        skill.name.to_string(),
                        skill.description.to_string(),
                    );
                    break;
                }
            }
        }
        *selected_skill_id.borrow_mut() = skill_id.clone();
        append_log(&logs, &format!("[skills] selected {skill_id}"));
        if let Some(app) = app_weak.upgrade() {
            app.set_orchestrator_selected_skill_id(skill_id.into());
            app.set_orchestrator_active_skill_instructions(instructions.into());
            app.set_orchestrator_active_log_output(logs_to_string(&logs).into());
        }
    });
}
