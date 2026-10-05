//! Inventory: suppliers, storage, products, purchases, movements, ledger, CSV, attachments.

use crate::attachments::{self, Attachment};
use crate::csvio::{self, CsvTable, ImportPreview, ProductMapping};
use crate::db::core::set_archived;
use crate::db::inventory::{self as q, *};
use crate::domain::units::{UnitDef, UNITS};
use crate::error::AppResult;
use crate::workspace::AppState;
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn units_list() -> Vec<UnitDef> {
    UNITS.to_vec()
}

#[tauri::command]
pub async fn suppliers_list(state: State<'_, AppState>) -> AppResult<Vec<Supplier>> {
    state.read(q::suppliers)
}
#[tauri::command]
pub async fn supplier_save(state: State<'_, AppState>, supplier: Supplier) -> AppResult<i64> {
    state.tx(|c| q::save_supplier(c, &supplier))
}
#[tauri::command]
pub async fn supplier_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| set_archived(c, "suppliers", "supplier", id, archived))
}
#[tauri::command]
pub async fn storage_list(state: State<'_, AppState>) -> AppResult<Vec<StorageLocation>> {
    state.read(q::storage_locations)
}
#[tauri::command]
pub async fn storage_save(state: State<'_, AppState>, storage: StorageLocation) -> AppResult<i64> {
    state.tx(|c| q::save_storage(c, &storage))
}
#[tauri::command]
pub async fn storage_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| set_archived(c, "storage_locations", "storage_location", id, archived))
}

#[tauri::command]
pub async fn products_list(state: State<'_, AppState>, include_archived: bool) -> AppResult<Vec<ProductRow>> {
    state.read(|c| q::products(c, include_archived))
}

#[derive(Serialize)]
pub struct ProductDetail {
    pub product: ProductRow,
    pub by_storage: Vec<StorageQty>,
    pub purchases: Vec<PurchaseRow>,
    pub ledger: Vec<LedgerRow>,
}

#[tauri::command]
pub async fn product_get(state: State<'_, AppState>, id: i64) -> AppResult<ProductDetail> {
    state.read(|c| {
        Ok(ProductDetail {
            product: q::product(c, id)?,
            by_storage: q::stock_by_storage(c, id)?,
            purchases: q::purchases(c, None, None, Some(id))?,
            ledger: q::ledger(c, &LedgerFilter { product_id: Some(id), limit: Some(300), ..Default::default() })?,
        })
    })
}

#[tauri::command]
pub async fn product_save(state: State<'_, AppState>, product: ProductInput) -> AppResult<ProductRow> {
    state.tx(|c| {
        let id = q::save_product(c, &product)?;
        q::product(c, id)
    })
}

#[tauri::command]
pub async fn products_bulk_update(state: State<'_, AppState>, ids: Vec<i64>, patch: BulkPatch) -> AppResult<usize> {
    state.tx(|c| q::bulk_update(c, &ids, &patch))
}

#[tauri::command]
pub async fn purchase_preview(state: State<'_, AppState>, purchase: PurchaseInput) -> AppResult<PurchasePreview> {
    state.read(|c| q::preview_purchase(c, &purchase))
}

#[tauri::command]
pub async fn purchase_receive(state: State<'_, AppState>, purchase: PurchaseInput) -> AppResult<i64> {
    state.tx(|c| q::receive(c, &purchase))
}

#[tauri::command]
pub async fn purchases_list(state: State<'_, AppState>, from: Option<String>, to: Option<String>) -> AppResult<Vec<PurchaseRow>> {
    state.read(|c| q::purchases(c, from.as_deref(), to.as_deref(), None))
}

#[tauri::command]
pub async fn purchase_reverse(state: State<'_, AppState>, id: i64, date: String, note: String) -> AppResult<()> {
    state.tx(|c| q::reverse_purchase(c, id, &date, &note))
}

#[tauri::command]
pub async fn movement_record(state: State<'_, AppState>, movement: MovementInput) -> AppResult<Option<Posted>> {
    state.tx(|c| q::record_movement(c, &movement))
}

#[tauri::command]
pub async fn ledger_list(state: State<'_, AppState>, filter: LedgerFilter) -> AppResult<Vec<LedgerRow>> {
    state.read(|c| q::ledger(c, &filter))
}

#[tauri::command]
pub async fn ledger_reverse(state: State<'_, AppState>, id: i64, date: String, note: String) -> AppResult<()> {
    state.tx(|c| q::reverse_entry(c, id, &date, &note))
}

#[tauri::command]
pub async fn reorder_list(state: State<'_, AppState>) -> AppResult<Vec<ReorderItem>> {
    state.read(q::reorder_list)
}

#[tauri::command]
pub async fn expiring_lots(state: State<'_, AppState>, until: String) -> AppResult<Vec<ExpiringLot>> {
    state.read(|c| q::expiring_lots(c, &until))
}

#[tauri::command]
pub async fn attachment_add(state: State<'_, AppState>, path: String) -> AppResult<Attachment> {
    let dir = state.current_attachments_dir()?;
    state.tx(|c| attachments::add(c, &dir, &PathBuf::from(&path)))
}

#[tauri::command]
pub async fn attachment_open(app: tauri::AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let dir = state.current_attachments_dir()?;
    let p = state.read(|c| attachments::path(c, &dir, id))?;
    app.opener()
        .open_path(p.to_string_lossy(), None::<&str>)
        .map_err(|e| crate::error::AppError::Other(format!("Couldn't open the file: {e}")))
}

#[tauri::command]
pub async fn csv_read(path: String) -> AppResult<CsvTable> {
    csvio::read(&PathBuf::from(path))
}

#[tauri::command]
pub async fn products_import_preview(state: State<'_, AppState>, path: String, mapping: ProductMapping) -> AppResult<ImportPreview> {
    let t = csvio::read(&PathBuf::from(path))?;
    state.read(|c| csvio::preview_products(c, &t, &mapping))
}

#[tauri::command]
pub async fn products_import_commit(state: State<'_, AppState>, path: String, mapping: ProductMapping, opening_date: String) -> AppResult<usize> {
    let t = csvio::read(&PathBuf::from(path))?;
    state.tx(|c| csvio::import_products(c, &t, &mapping, &opening_date))
}

#[tauri::command]
pub async fn products_export(state: State<'_, AppState>, path: String) -> AppResult<usize> {
    let p = state.output_path(&path, "csv")?;
    state.read(|c| csvio::export_products(c, &p))
}

#[tauri::command]
pub async fn ledger_export(state: State<'_, AppState>, path: String, filter: LedgerFilter) -> AppResult<usize> {
    let p = state.output_path(&path, "csv")?;
    state.read(|c| csvio::export_ledger(c, &p, &filter))
}
