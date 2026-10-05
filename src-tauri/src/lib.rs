pub mod attachments;
pub mod backup;
pub mod csvio;
pub mod commands;
pub mod db;
pub mod demo;
pub mod domain;
pub mod error;
pub mod gazetteer;
pub mod providers;
pub mod workspace;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let root = workspace::data_root(&app.path().app_data_dir()?);
            app.manage(workspace::AppState::new(root)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_status,
            commands::app::workspace_create,
            commands::app::workspace_open,
            commands::app::workspace_close,
            commands::app::workspace_rename,
            commands::app::workspace_delete,
            commands::app::backup_list,
            commands::app::backup_create,
            commands::app::backup_inspect,
            commands::app::backup_restore,
            commands::app::demo_create,
            commands::app::export_all,
            commands::core::business_get,
            commands::core::business_save,
            commands::core::locations_list,
            commands::core::location_save,
            commands::core::location_archive,
            commands::core::staff_list,
            commands::core::staff_save,
            commands::core::staff_archive,
            commands::core::audit_list,
            commands::profiles::profiles_list,
            commands::profiles::profile_get,
            commands::profiles::profile_versions,
            commands::profiles::profile_preview,
            commands::profiles::profile_save,
            commands::profiles::profile_archive,
            commands::inventory::units_list,
            commands::inventory::suppliers_list,
            commands::inventory::supplier_save,
            commands::inventory::supplier_archive,
            commands::inventory::storage_list,
            commands::inventory::storage_save,
            commands::inventory::storage_archive,
            commands::inventory::products_list,
            commands::inventory::product_get,
            commands::inventory::product_save,
            commands::inventory::products_bulk_update,
            commands::inventory::purchase_preview,
            commands::inventory::purchase_receive,
            commands::inventory::purchases_list,
            commands::inventory::purchase_reverse,
            commands::inventory::movement_record,
            commands::inventory::ledger_list,
            commands::inventory::ledger_reverse,
            commands::inventory::reorder_list,
            commands::inventory::expiring_lots,
            commands::inventory::attachment_add,
            commands::inventory::attachment_open,
            commands::inventory::csv_read,
            commands::inventory::products_import_preview,
            commands::inventory::products_import_commit,
            commands::inventory::products_export,
            commands::inventory::ledger_export,
            commands::services::services_list,
            commands::services::service_get,
            commands::services::service_save,
            commands::services::service_archive,
            commands::services::service_estimate,
            commands::services::pricing_what_if,
            commands::services::bundles_list,
            commands::services::bundle_save,
            commands::services::bundle_archive,
            commands::tax::tax_overview,
            commands::tax::tax_categories,
            commands::tax::tax_category_add,
            commands::tax::tax_rule_set,
            commands::tax::tax_rate_save,
            commands::tax::tax_rate_clear,
            commands::tax::tax_rate_history,
            commands::providers::providers_status,
            commands::providers::provider_key_set,
            commands::providers::provider_key_clear,
            commands::providers::tax_lookup,
            commands::providers::tax_rate_apply_lookup,
            commands::providers::location_geocode,
            commands::reports::report_run,
            commands::reports::report_drill,
            commands::reports::report_tax,
            commands::reports::dashboards_list,
            commands::reports::dashboard_save,
            commands::reports::dashboard_delete,
            commands::reports::dashboard_snapshot,
            commands::reports::export_csv,
            commands::reports::competitors_list,
            commands::reports::competitor_save,
            commands::reports::competitor_archive,
            commands::reports::observations_list,
            commands::reports::observation_save,
            commands::reports::observation_exclude,
            commands::reports::observation_delete,
            commands::reports::market_evidence,
            commands::reports::observations_import_preview,
            commands::reports::observations_import_commit,
            commands::reports::competitors_discover,
            commands::reports::market_context,
            commands::sales::clients_list,
            commands::sales::client_get,
            commands::sales::client_save,
            commands::sales::client_archive,
            commands::sales::formula_save,
            commands::sales::appointments_list,
            commands::sales::appointment_get,
            commands::sales::appointment_save,
            commands::sales::appointment_reschedule,
            commands::sales::appointment_status,
            commands::sales::appointment_minutes,
            commands::sales::estimates_list,
            commands::sales::estimate_get,
            commands::sales::estimate_save,
            commands::sales::estimate_status,
            commands::sales::estimate_default_price,
            commands::sales::sales_list,
            commands::sales::sale_get,
            commands::sales::sale_save_draft,
            commands::sales::sale_delete_draft,
            commands::sales::sale_from_appointment,
            commands::sales::sale_finalize,
            commands::sales::sale_void,
            commands::sales::sale_refund,
            commands::sales::payment_add,
            commands::sales::payment_void,
            commands::sales::expense_categories,
            commands::sales::expenses_list,
            commands::sales::expense_save,
            commands::sales::expense_void,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Salon Resource Manager");
}
