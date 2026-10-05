//! Appointments: a staff member's time with a client for one or more services. Times are local
//! business times ("YYYY-MM-DDTHH:MM"). Double-booking a staff member needs explicit confirmation.

use super::core::get_business;
use super::services;
use super::{audit, now_utc};
use crate::error::{AppError, AppResult};
use chrono::{Datelike, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppointmentLine {
    pub service_id: i64,
    pub variant_ids: Vec<i64>,
    pub qty: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppointmentInput {
    pub id: Option<i64>,
    pub client_id: Option<i64>,
    pub staff_id: i64,
    pub location_id: Option<i64>,
    pub starts_at: String,
    /// None = computed from the services' chair time
    pub ends_at: Option<String>,
    pub notes: String,
    pub lines: Vec<AppointmentLine>,
    pub estimate_id: Option<i64>,
    /// Save even if it overlaps another booking for the same person.
    pub allow_overlap: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct AppointmentView {
    pub id: i64,
    pub client_id: Option<i64>,
    pub client_name: Option<String>,
    pub staff_id: i64,
    pub staff_name: String,
    pub staff_color: String,
    pub location_id: Option<i64>,
    pub starts_at: String,
    pub ends_at: String,
    pub status: String,
    pub notes: String,
    pub lines: Vec<AppointmentLine>,
    pub service_names: Vec<String>,
    pub sale_id: Option<i64>,
    pub sale_status: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct SaveResult {
    pub id: Option<i64>,
    pub conflicts: Vec<String>,
    pub warnings: Vec<String>,
}

const FMT: &str = "%Y-%m-%dT%H:%M";

fn parse(field: &str, s: &str) -> AppResult<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, FMT).map_err(|_| AppError::invalid(field, "Use a date and time like 2026-10-05T14:30."))
}

/// Chair time for the booked services (variants included).
pub fn planned_minutes(conn: &Connection, lines: &[AppointmentLine]) -> AppResult<i64> {
    let mut total = 0;
    for l in lines {
        let s = services::get(conn, l.service_id)?;
        let (t, _, _, _) = services::apply_variants(&s.input, &l.variant_ids)?;
        total += t.occupied_min() * l.qty;
    }
    Ok(total)
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<AppointmentView> {
    Ok(AppointmentView {
        id: r.get(0)?,
        client_id: r.get(1)?,
        client_name: r.get::<_, Option<String>>(2)?.map(|s| s.trim().to_string()),
        staff_id: r.get(3)?,
        staff_name: r.get(4)?,
        staff_color: r.get(5)?,
        location_id: r.get(6)?,
        starts_at: r.get(7)?,
        ends_at: r.get(8)?,
        status: r.get(9)?,
        notes: r.get(10)?,
        lines: vec![],
        service_names: vec![],
        sale_id: r.get(11)?,
        sale_status: r.get(12)?,
    })
}

const SELECT: &str = "SELECT a.id, a.client_id, c.first_name || ' ' || c.last_name, a.staff_id, st.name, st.color, a.location_id, a.starts_at, a.ends_at, a.status, a.notes,
    s.id, s.status FROM appointments a JOIN staff st ON st.id = a.staff_id LEFT JOIN clients c ON c.id = a.client_id LEFT JOIN sales s ON s.appointment_id = a.id";

fn fill_lines(conn: &Connection, v: &mut AppointmentView) -> AppResult<()> {
    let mut st = conn.prepare("SELECT l.service_id, l.variant_ids, l.qty, sv.name FROM appointment_lines l JOIN services sv ON sv.id = l.service_id WHERE l.appointment_id = ?1 ORDER BY l.sort, l.id")?;
    let rows: Vec<(AppointmentLine, String)> = st
        .query_map([v.id], |r| {
            Ok((AppointmentLine { service_id: r.get(0)?, variant_ids: serde_json::from_str(&r.get::<_, String>(1)?).unwrap_or_default(), qty: r.get(2)? }, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    v.service_names = rows.iter().map(|(_, n)| n.clone()).collect();
    v.lines = rows.into_iter().map(|(l, _)| l).collect();
    Ok(())
}

pub fn get(conn: &Connection, id: i64) -> AppResult<AppointmentView> {
    let mut v = conn
        .query_row(&format!("{SELECT} WHERE a.id = ?1"), [id], row)
        .optional()?
        .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
    fill_lines(conn, &mut v)?;
    Ok(v)
}

pub fn list(conn: &Connection, from: &str, to: &str, staff_id: Option<i64>, client_id: Option<i64>) -> AppResult<Vec<AppointmentView>> {
    let mut st = conn.prepare(&format!(
        "{SELECT} WHERE a.starts_at < ?2 AND a.ends_at > ?1 AND (?3 IS NULL OR a.staff_id = ?3) AND (?4 IS NULL OR a.client_id = ?4) ORDER BY a.starts_at"
    ))?;
    let mut out: Vec<AppointmentView> = st.query_map(params![from, to, staff_id, client_id], row)?.collect::<Result<_, _>>()?;
    for v in out.iter_mut() {
        fill_lines(conn, v)?;
    }
    Ok(out)
}

pub fn conflicts(conn: &Connection, staff_id: i64, starts: &str, ends: &str, exclude: Option<i64>) -> AppResult<Vec<String>> {
    let mut st = conn.prepare(
        "SELECT a.starts_at, a.ends_at, TRIM(COALESCE(c.first_name || ' ' || c.last_name, 'Walk-in')) FROM appointments a LEFT JOIN clients c ON c.id = a.client_id
         WHERE a.staff_id = ?1 AND a.status IN ('scheduled','checked_in') AND a.starts_at < ?3 AND a.ends_at > ?2 AND (?4 IS NULL OR a.id <> ?4) ORDER BY a.starts_at",
    )?;
    let rows = st.query_map(params![staff_id, starts, ends, exclude], |r| {
        let s: String = r.get(0)?;
        let e: String = r.get(1)?;
        let who: String = r.get(2)?;
        Ok(format!("{} {}–{} with {}", &s[..10], &s[11..16], &e[11..16], if who.is_empty() { "walk-in".into() } else { who }))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn save(conn: &Connection, a: &AppointmentInput) -> AppResult<SaveResult> {
    let start = parse("starts_at", &a.starts_at)?;
    if a.lines.is_empty() {
        return Err(AppError::invalid("lines", "Add at least one service."));
    }
    for l in &a.lines {
        if l.qty < 1 {
            return Err(AppError::invalid("lines", "Quantities must be at least 1."));
        }
    }
    let minutes = planned_minutes(conn, &a.lines)?;
    let end = match &a.ends_at {
        Some(e) => parse("ends_at", e)?,
        None => start + chrono::Duration::minutes(minutes.max(5)),
    };
    if end <= start {
        return Err(AppError::invalid("ends_at", "The end time must be after the start."));
    }
    let (s, e) = (start.format(FMT).to_string(), end.format(FMT).to_string());
    if let Some(id) = a.id {
        let status: String = conn.query_row("SELECT status FROM appointments WHERE id = ?1", [id], |r| r.get(0))?;
        if status == "completed" {
            return Err(AppError::msg("A completed appointment can't be changed; its sale is the record."));
        }
    }
    let conflicts = conflicts(conn, a.staff_id, &s, &e, a.id)?;
    let mut warnings = Vec::new();
    let biz = get_business(conn)?;
    let day = biz.schedule.iter().find(|d| d.day as u32 == start.weekday().num_days_from_monday());
    match day {
        Some(d) if !d.open => warnings.push("This is outside your opening days.".into()),
        Some(d) if s[11..16] < *d.start || e[11..16] > *d.end || s[..10] != e[..10] => warnings.push(format!("This runs outside opening hours ({}–{}).", d.start, d.end)),
        _ => {}
    }
    if !conflicts.is_empty() && !a.allow_overlap {
        return Ok(SaveResult { id: None, conflicts, warnings });
    }
    let id = match a.id {
        Some(id) => {
            conn.execute(
                "UPDATE appointments SET client_id=?1, staff_id=?2, location_id=?3, starts_at=?4, ends_at=?5, notes=?6, updated_at=?7 WHERE id=?8",
                params![a.client_id, a.staff_id, a.location_id, s, e, a.notes, now_utc(), id],
            )?;
            conn.execute("DELETE FROM appointment_lines WHERE appointment_id = ?1", [id])?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO appointments (client_id, staff_id, location_id, starts_at, ends_at, notes, estimate_id) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![a.client_id, a.staff_id, a.location_id.or(biz.primary_location_id), s, e, a.notes, a.estimate_id],
            )?;
            conn.last_insert_rowid()
        }
    };
    for (i, l) in a.lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO appointment_lines (appointment_id, service_id, variant_ids, qty, sort) VALUES (?1,?2,?3,?4,?5)",
            params![id, l.service_id, serde_json::to_string(&l.variant_ids)?, l.qty, i as i64],
        )?;
    }
    if let Some(est) = a.estimate_id {
        conn.execute("UPDATE estimates SET status = 'converted' WHERE id = ?1", [est])?;
    }
    audit(
        conn,
        "appointment",
        Some(id),
        if a.id.is_some() { "update" } else { "create" },
        &format!("Appointment {s}–{}{}", &e[11..16], if conflicts.is_empty() { "" } else { " (overlap confirmed)" }),
        None,
    )?;
    Ok(SaveResult { id: Some(id), conflicts, warnings })
}

/// Move an appointment (drag-and-drop), keeping its length unless an end is given.
pub fn reschedule(conn: &Connection, id: i64, starts_at: &str, ends_at: Option<&str>, staff_id: Option<i64>, allow_overlap: bool) -> AppResult<SaveResult> {
    let cur = get(conn, id)?;
    let old_start = parse("starts_at", &cur.starts_at)?;
    let old_end = parse("ends_at", &cur.ends_at)?;
    let new_start = parse("starts_at", starts_at)?;
    let new_end = match ends_at {
        Some(e) => parse("ends_at", e)?,
        None => new_start + (old_end - old_start),
    };
    save(
        conn,
        &AppointmentInput {
            id: Some(id),
            client_id: cur.client_id,
            staff_id: staff_id.unwrap_or(cur.staff_id),
            location_id: cur.location_id,
            starts_at: new_start.format(FMT).to_string(),
            ends_at: Some(new_end.format(FMT).to_string()),
            notes: cur.notes,
            lines: cur.lines,
            estimate_id: None,
            allow_overlap,
        },
    )
}

pub fn set_status(conn: &Connection, id: i64, status: &str) -> AppResult<()> {
    if !matches!(status, "scheduled" | "checked_in" | "cancelled" | "no_show") {
        return Err(AppError::invalid("status", "Completing happens by finalizing the appointment's sale."));
    }
    let has_final_sale: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM sales WHERE appointment_id = ?1 AND status = 'finalized')", [id], |r| r.get(0))?;
    if has_final_sale {
        return Err(AppError::msg("This appointment's sale is finalized. Refund or void the sale instead."));
    }
    let n = conn.execute("UPDATE appointments SET status = ?1, updated_at = ?2 WHERE id = ?3", params![status, now_utc(), id])?;
    if n == 0 {
        return Err(AppError::NotFound("Appointment not found.".into()));
    }
    audit(conn, "appointment", Some(id), "status", &format!("Appointment marked {status}"), None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sales::tests::fixture;

    fn input(f: &crate::db::sales::tests::Fixture, start: &str) -> AppointmentInput {
        AppointmentInput {
            id: None,
            client_id: None,
            staff_id: f.staff,
            location_id: Some(f.loc),
            starts_at: start.into(),
            ends_at: None,
            notes: String::new(),
            lines: vec![AppointmentLine { service_id: f.service, variant_ids: vec![], qty: 1 }],
            estimate_id: None,
            allow_overlap: false,
        }
    }

    #[test]
    fn duration_conflicts_and_reschedule() {
        let f = fixture();
        // 2026-10-05 is a Monday; service chair time is 95 min
        let a = save(&f.conn, &input(&f, "2026-10-05T10:00")).unwrap();
        let id = a.id.unwrap();
        assert_eq!(get(&f.conn, id).unwrap().ends_at, "2026-10-05T11:35");
        let clash = save(&f.conn, &input(&f, "2026-10-05T11:00")).unwrap();
        assert!(clash.id.is_none());
        assert_eq!(clash.conflicts.len(), 1);
        let ok = save(&f.conn, &AppointmentInput { allow_overlap: true, ..input(&f, "2026-10-05T11:00") }).unwrap();
        assert!(ok.id.is_some());
        let r = reschedule(&f.conn, id, "2026-10-05T13:00", None, None, false).unwrap();
        assert!(r.conflicts.is_empty());
        assert_eq!(get(&f.conn, id).unwrap().ends_at, "2026-10-05T14:35");
        // outside hours warns
        let late = save(&f.conn, &input(&f, "2026-10-05T16:30")).unwrap();
        assert!(late.warnings.iter().any(|w| w.contains("outside opening hours")));
        // cancelled appointments don't block
        set_status(&f.conn, late.id.unwrap(), "cancelled").unwrap();
        assert!(conflicts(&f.conn, f.staff, "2026-10-05T16:30", "2026-10-05T17:00", None).unwrap().is_empty());
    }

    #[test]
    fn checkout_from_appointment_is_idempotent() {
        let f = fixture();
        let id = save(&f.conn, &input(&f, "2026-10-05T10:00")).unwrap().id.unwrap();
        let s1 = crate::db::sales::draft_from_appointment(&f.conn, id).unwrap();
        let s2 = crate::db::sales::draft_from_appointment(&f.conn, id).unwrap();
        assert_eq!(s1, s2);
        let v = crate::db::sales::get(&f.conn, s1).unwrap();
        assert_eq!(v.draft.lines[0].usage.len(), 1); // planned usage from the recipe
        assert_eq!(get(&f.conn, id).unwrap().status, "checked_in");
    }
}
