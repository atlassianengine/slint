// Rust guideline compliant 2026-02-21

//! Presentation mapping logic converting `planner-core` domain models into Slint UI structs.

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use planner_core::{
    parse_day_key, CalendarDayCell, PlannerItem, PlannerViewMode, YearMonthSummary,
};
use slint::{Model, ModelRc, VecModel};
use std::collections::HashMap;

use crate::{
    color::accent_color, PlannerCalendarCell, PlannerDayColumn, PlannerItem as SlintPlannerItem,
    PlannerViewMode as SlintViewMode, PlannerYearSummary,
};

/// Converts a domain `PlannerViewMode` into a Slint UI `PlannerViewMode`.
pub fn view_mode_to_slint(mode: PlannerViewMode) -> SlintViewMode {
    match mode {
        PlannerViewMode::Daily => SlintViewMode::Daily,
        PlannerViewMode::Weekly => SlintViewMode::Weekly,
        PlannerViewMode::Monthly => SlintViewMode::Monthly,
        PlannerViewMode::Yearly => SlintViewMode::Yearly,
    }
}

/// Converts a Slint UI `PlannerViewMode` into a domain `PlannerViewMode`.
pub fn view_mode_from_slint(mode: SlintViewMode) -> PlannerViewMode {
    match mode {
        SlintViewMode::Daily => PlannerViewMode::Daily,
        SlintViewMode::Weekly => PlannerViewMode::Weekly,
        SlintViewMode::Monthly => PlannerViewMode::Monthly,
        SlintViewMode::Yearly => PlannerViewMode::Yearly,
    }
}

fn to_slint_item(item: &PlannerItem) -> SlintPlannerItem {
    SlintPlannerItem {
        id: item.id.clone().into(),
        title: item.title.clone().into(),
        subtitle: item.subtitle.clone().into(),
        role: "Task".into(),
        day_key: item.day_key.clone().into(),
        start_time_mins: item.start_time_mins as i32,
        duration_mins: item.duration_mins as i32,
        accent: accent_color(&item.accent_hex),
        completed: item.completed,
        selected: item.selected,
    }
}

fn day_key_density_map(items: &[PlannerItem]) -> HashMap<String, i32> {
    let mut density = HashMap::new();
    for item in items {
        *density.entry(item.day_key.clone()).or_insert(0) += 1;
    }
    density
}

/// Map a list of domain `PlannerItem`s into a Slint `ModelRc`.
pub fn items_to_slint_model(items: &[PlannerItem]) -> ModelRc<SlintPlannerItem> {
    let rows: Vec<SlintPlannerItem> = items.iter().map(to_slint_item).collect();
    ModelRc::new(VecModel::from(rows))
}

/// Builds weekly columns keyed by day key for the 7-day planner view.
pub fn week_day_items(week_day_keys: &[String], items: &[PlannerItem]) -> Vec<PlannerDayColumn> {
    week_day_keys
        .iter()
        .map(|day_key| PlannerDayColumn {
            id: day_key.clone().into(),
            name: parse_day_key(day_key)
                .map(|day| day.format("%a %d").to_string())
                .unwrap_or_else(|| day_key.clone())
                .into(),
            items: ModelRc::new(VecModel::from(
                items
                    .iter()
                    .filter(|item| item.day_key == *day_key)
                    .map(to_slint_item)
                    .collect::<Vec<SlintPlannerItem>>(),
            )),
        })
        .collect()
}

/// Builds daily columns (default demo lanes) for task-card layout.
pub fn daily_columns(items: &[PlannerItem], active_day_key: &str) -> Vec<PlannerDayColumn> {
    let mut columns: Vec<PlannerDayColumn> = ["Focus", "Admin", "Maintenance"]
        .iter()
        .map(|name| PlannerDayColumn {
            id: (*name).to_string().into(),
            name: (*name).to_string().into(),
            items: ModelRc::new(VecModel::from(Vec::<SlintPlannerItem>::new())),
        })
        .collect();

    for (index, item) in items
        .iter()
        .filter(|item| item.day_key == active_day_key)
        .enumerate()
    {
        let target = index % columns.len();
        if let Some(column) = columns.get_mut(target) {
            column.items.push_row(to_slint_item(item));
        }
    }

    columns
}

/// Maps month calendar grid cells into a Slint `ModelRc`.
pub fn month_cells_to_slint_model(
    cells: &[CalendarDayCell],
    items: &[PlannerItem],
) -> ModelRc<PlannerCalendarCell> {
    let density = day_key_density_map(items);
    let rows: Vec<PlannerCalendarCell> = cells
        .iter()
        .map(|cell| PlannerCalendarCell {
            day_key: cell.day_key.clone().into(),
            day_number: if cell.is_current_month {
                cell.day_number as i32
            } else {
                0
            },
            density: *density.get(&cell.day_key).unwrap_or(&0),
            is_current_month: cell.is_current_month,
            is_today: cell.is_today,
            is_weekend: cell.is_weekend,
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

/// Maps 12-month yearly summaries into a Slint `ModelRc`.
pub fn year_summaries_to_slint_model(
    summaries: &[YearMonthSummary],
) -> ModelRc<PlannerYearSummary> {
    let rows: Vec<PlannerYearSummary> = summaries
        .iter()
        .map(|s| PlannerYearSummary {
            month_number: s.month_number as i32,
            month_name: s.month_name.clone().into(),
            total_days: s.total_days as i32,
            start_weekday: s.start_weekday as i32,
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

/// Builds dense year cells (12 months × 42 cells) including density.
pub fn year_cells_to_slint_model(
    year: i32,
    today: NaiveDate,
    items: &[PlannerItem],
) -> Vec<PlannerCalendarCell> {
    let density = day_key_density_map(items);
    let mut rows = Vec::new();

    for month in 1..=12 {
        let first_of_month = NaiveDate::from_ymd_opt(year, month as u32, 1).unwrap();
        let start_weekday = first_of_month.weekday().num_days_from_monday();
        let grid_start = first_of_month - Duration::days(i64::from(start_weekday));

        for day_offset in 0..42 {
            let date = grid_start + Duration::days(day_offset as i64);
            let day_key = date.format("%Y-%m-%d").to_string();
            let is_current_month = date.month() == month as u32 && date.year() == year;
            rows.push(PlannerCalendarCell {
                day_key: day_key.clone().into(),
                day_number: if is_current_month {
                    date.day() as i32
                } else {
                    0
                },
                density: *density.get(&day_key).unwrap_or(&0),
                is_current_month,
                is_today: date == today,
                is_weekend: matches!(date.weekday(), Weekday::Sat | Weekday::Sun),
            });
        }
    }

    rows
}
