//! Clients, appointments, estimates, sales, payments, refunds and expenses.

use crate::db::appointments::{self as appts, AppointmentInput, AppointmentView, SaveResult};
use crate::db::clients::{self, ClientDetail, ClientInput, ClientRow, FormulaInput};
use crate::db::core::set_archived;
use crate::db::estimates::{self, EstimateInput, EstimateView};
use crate::db::expenses::{self, Expense, ExpenseInput};
use crate::db::sales::{self, PaymentInput, RefundInput, SaleDraft, SaleFilter, SaleSummary, SaleView};
use crate::error::AppResult;
use crate::workspace::AppState;
use rust_decimal::Decimal;
use tauri::State;

fn now_local() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string()
}

// clients
#[tauri::command]
pub async fn clients_list(state: State<'_, AppState>, include_archived: bool) -> AppResult<Vec<ClientRow>> {
    state.read(|c| clients::list(c, include_archived, &now_local()))
}
#[tauri::command]
pub async fn client_get(state: State<'_, AppState>, id: i64) -> AppResult<ClientDetail> {
    state.read(|c| clients::detail(c, id, &now_local()))
}
#[tauri::command]
pub async fn client_save(state: State<'_, AppState>, client: ClientInput) -> AppResult<i64> {
    state.tx(|c| clients::save(c, &client))
}
#[tauri::command]
pub async fn client_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| set_archived(c, "clients", "client", id, archived))
}
#[tauri::command]
pub async fn formula_save(state: State<'_, AppState>, formula: FormulaInput) -> AppResult<i64> {
    state.tx(|c| clients::save_formula(c, &formula))
}

// appointments
#[tauri::command]
pub async fn appointments_list(state: State<'_, AppState>, from: String, to: String, staff_id: Option<i64>, client_id: Option<i64>) -> AppResult<Vec<AppointmentView>> {
    state.read(|c| appts::list(c, &from, &to, staff_id, client_id))
}
#[tauri::command]
pub async fn appointment_get(state: State<'_, AppState>, id: i64) -> AppResult<AppointmentView> {
    state.read(|c| appts::get(c, id))
}
#[tauri::command]
pub async fn appointment_save(state: State<'_, AppState>, appointment: AppointmentInput) -> AppResult<SaveResult> {
    state.tx(|c| appts::save(c, &appointment))
}
#[tauri::command]
pub async fn appointment_reschedule(state: State<'_, AppState>, id: i64, starts_at: String, ends_at: Option<String>, staff_id: Option<i64>, allow_overlap: bool) -> AppResult<SaveResult> {
    state.tx(|c| appts::reschedule(c, id, &starts_at, ends_at.as_deref(), staff_id, allow_overlap))
}
#[tauri::command]
pub async fn appointment_status(state: State<'_, AppState>, id: i64, status: String) -> AppResult<()> {
    state.tx(|c| appts::set_status(c, id, &status))
}
#[tauri::command]
pub async fn appointment_minutes(state: State<'_, AppState>, lines: Vec<appts::AppointmentLine>) -> AppResult<i64> {
    state.read(|c| appts::planned_minutes(c, &lines))
}

// estimates
#[tauri::command]
pub async fn estimates_list(state: State<'_, AppState>) -> AppResult<Vec<EstimateView>> {
    state.read(estimates::list)
}
#[tauri::command]
pub async fn estimate_get(state: State<'_, AppState>, id: i64) -> AppResult<EstimateView> {
    state.read(|c| estimates::get(c, id))
}
#[tauri::command]
pub async fn estimate_save(state: State<'_, AppState>, estimate: EstimateInput) -> AppResult<i64> {
    state.tx(|c| estimates::save(c, &estimate))
}
#[tauri::command]
pub async fn estimate_status(state: State<'_, AppState>, id: i64, status: String) -> AppResult<()> {
    state.tx(|c| estimates::set_status(c, id, &status))
}
#[tauri::command]
pub async fn estimate_default_price(state: State<'_, AppState>, service_id: i64, variant_ids: Vec<i64>) -> AppResult<Decimal> {
    state.read(|c| estimates::default_price(c, service_id, &variant_ids))
}

// sales
#[tauri::command]
pub async fn sales_list(state: State<'_, AppState>, filter: SaleFilter) -> AppResult<Vec<SaleSummary>> {
    state.read(|c| sales::list(c, &filter))
}
#[tauri::command]
pub async fn sale_get(state: State<'_, AppState>, id: i64) -> AppResult<SaleView> {
    state.read(|c| sales::get(c, id))
}
#[tauri::command]
pub async fn sale_save_draft(state: State<'_, AppState>, sale: SaleDraft) -> AppResult<SaleView> {
    state.tx(|c| {
        let id = sales::save_draft(c, &sale)?;
        sales::get(c, id)
    })
}
#[tauri::command]
pub async fn sale_delete_draft(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.tx(|c| sales::delete_draft(c, id))
}
#[tauri::command]
pub async fn sale_from_appointment(state: State<'_, AppState>, appointment_id: i64) -> AppResult<i64> {
    state.tx(|c| sales::draft_from_appointment(c, appointment_id))
}
#[tauri::command]
pub async fn sale_finalize(state: State<'_, AppState>, id: i64) -> AppResult<SaleView> {
    state.tx(|c| sales::finalize(c, id))
}
#[tauri::command]
pub async fn sale_void(state: State<'_, AppState>, id: i64, reason: String) -> AppResult<()> {
    state.tx(|c| sales::void(c, id, &reason))
}
#[tauri::command]
pub async fn sale_refund(state: State<'_, AppState>, id: i64, refund: RefundInput) -> AppResult<i64> {
    state.tx(|c| sales::refund(c, id, &refund))
}
#[tauri::command]
pub async fn payment_add(state: State<'_, AppState>, sale_id: i64, payment: PaymentInput) -> AppResult<i64> {
    state.tx(|c| sales::add_payment(c, sale_id, &payment))
}
#[tauri::command]
pub async fn payment_void(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.tx(|c| sales::void_payment(c, id))
}

// expenses
#[tauri::command]
pub async fn expense_categories() -> Vec<(String, String)> {
    expenses::CATEGORIES.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}
#[tauri::command]
pub async fn expenses_list(state: State<'_, AppState>, from: Option<String>, to: Option<String>) -> AppResult<Vec<Expense>> {
    state.read(|c| expenses::list(c, from.as_deref(), to.as_deref()))
}
#[tauri::command]
pub async fn expense_save(state: State<'_, AppState>, expense: ExpenseInput) -> AppResult<i64> {
    state.tx(|c| expenses::save(c, &expense))
}
#[tauri::command]
pub async fn expense_void(state: State<'_, AppState>, id: i64, reason: String) -> AppResult<()> {
    state.tx(|c| expenses::void(c, id, &reason))
}
