//! CSV reading/writing and the product import (column mapping → validated preview → commit).

use crate::db::inventory::{self, ProductInput, PurchaseInput, PurchaseLineInput};
use crate::domain::units;
use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_ROWS: usize = 50_000;

#[derive(Serialize, Debug, Clone)]
pub struct CsvTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn read(path: &Path) -> AppResult<CsvTable> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_path(path)
        .map_err(|e| AppError::msg(format!("Couldn't open the CSV file: {e}")))?;
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| AppError::msg(format!("Couldn't read the CSV header row: {e}")))?
        .iter()
        .map(|h| h.trim_start_matches('\u{feff}').to_string())
        .collect();
    let mut rows = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        if i >= MAX_ROWS {
            return Err(AppError::msg(format!("This file has more than {MAX_ROWS} rows. Split it into smaller files.")));
        }
        let rec = rec.map_err(|e| AppError::msg(format!("Row {} can't be read: {e}", i + 2)))?;
        if rec.iter().all(|c| c.is_empty()) {
            continue;
        }
        rows.push(rec.iter().map(String::from).collect());
    }
    Ok(CsvTable { headers, rows })
}

pub fn write(path: &Path, headers: &[&str], rows: impl IntoIterator<Item = Vec<String>>) -> AppResult<usize> {
    let tmp = path.with_extension("csv.partial");
    let mut n = 0;
    {
        let mut w = csv::Writer::from_path(&tmp).map_err(|e| AppError::msg(format!("Couldn't create the file: {e}")))?;
        w.write_record(headers).map_err(|e| AppError::Other(e.to_string()))?;
        for r in rows {
            w.write_record(r.iter().map(|c| sanitize(c))).map_err(|e| AppError::Other(e.to_string()))?;
            n += 1;
        }
        w.flush()?;
    }
    std::fs::rename(tmp, path)?;
    Ok(n)
}

/// Neutralize spreadsheet formula injection in exported cells.
fn sanitize(c: &str) -> String {
    if c.starts_with(['=', '+', '@']) || (c.starts_with('-') && c.parse::<f64>().is_err()) {
        format!("'{c}")
    } else {
        c.to_string()
    }
}

/// Column index for each importable field (None = not mapped).
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct ProductMapping {
    pub name: Option<usize>,
    pub brand: Option<usize>,
    pub category: Option<usize>,
    pub subcategory: Option<usize>,
    pub sku: Option<usize>,
    pub barcode: Option<usize>,
    pub supplier: Option<usize>,
    pub stock_unit: Option<usize>,
    pub density_g_per_ml: Option<usize>,
    pub reorder_point: Option<usize>,
    pub retail_price: Option<usize>,
    pub opening_qty: Option<usize>,
    pub opening_unit_cost: Option<usize>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ImportRow {
    pub line: usize,
    /// "new" | "duplicate" | "error"
    pub status: String,
    pub message: Option<String>,
    pub name: String,
    pub brand: String,
    pub category: String,
    pub sku: String,
    pub stock_unit: String,
    pub opening_qty: Option<Decimal>,
    pub opening_unit_cost: Option<Decimal>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ImportPreview {
    pub rows: Vec<ImportRow>,
    pub new_count: usize,
    pub duplicate_count: usize,
    pub error_count: usize,
}

fn cell(row: &[String], i: Option<usize>) -> String {
    i.and_then(|i| row.get(i)).cloned().unwrap_or_default().trim().to_string()
}

fn parse_dec(s: &str, what: &str) -> Result<Option<Decimal>, String> {
    let t = s.trim().trim_start_matches('$').replace(',', "");
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<Decimal>().map(Some).map_err(|_| format!("{what} \"{s}\" isn't a number"))
}

/// Accepts unit codes and common spellings ("fl oz", "oz", "each").
pub fn normalize_unit(s: &str) -> Option<&'static str> {
    let t = s.trim().to_lowercase().replace('.', "");
    Some(match t.as_str() {
        "g" | "gram" | "grams" => "g",
        "kg" | "kilogram" | "kilograms" => "kg",
        "oz" | "oz_wt" | "ounce" | "ounces" | "oz wt" | "wt oz" => "oz_wt",
        "lb" | "lbs" | "pound" | "pounds" => "lb",
        "ml" | "milliliter" | "millilitre" | "milliliters" | "millilitres" => "ml",
        "l" | "liter" | "litre" | "liters" | "litres" => "l",
        "fl oz" | "fl_oz" | "floz" | "fluid ounce" | "fluid ounces" => "fl_oz",
        "gal" | "gallon" | "gallons" => "gal",
        "piece" | "pieces" | "each" | "ea" | "pc" | "pcs" | "unit" | "units" => "piece",
        _ => return None,
    })
}

fn normalize_category(s: &str) -> Option<&'static str> {
    match s.trim().to_lowercase().as_str() {
        "" | "professional" | "pro" | "backbar" | "back bar" => Some("professional"),
        "consumable" | "consumables" | "disposable" | "disposables" => Some("consumable"),
        "retail" => Some("retail"),
        _ => None,
    }
}

struct Parsed {
    input: ProductInput,
    supplier: String,
    opening_qty: Option<Decimal>,
    opening_unit_cost: Option<Decimal>,
}

fn parse_rows(conn: &Connection, table: &CsvTable, m: &ProductMapping) -> AppResult<(Vec<ImportRow>, Vec<Option<Parsed>>)> {
    if m.name.is_none() || m.stock_unit.is_none() {
        return Err(AppError::msg("Map at least the product name and the stock unit columns."));
    }
    let existing = inventory::products(conn, true)?;
    let skus: HashSet<String> = existing.iter().filter(|p| !p.sku.is_empty()).map(|p| p.sku.to_lowercase()).collect();
    let barcodes: HashSet<String> = existing.iter().filter(|p| !p.barcode.is_empty()).map(|p| p.barcode.clone()).collect();
    let names: HashSet<(String, String)> = existing.iter().map(|p| (p.name.to_lowercase(), p.brand.to_lowercase())).collect();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    let mut parsed = Vec::new();
    for (i, row) in table.rows.iter().enumerate() {
        let line = i + 2;
        let name = cell(row, m.name);
        let brand = cell(row, m.brand);
        let sku = cell(row, m.sku);
        let barcode = cell(row, m.barcode);
        let unit_raw = cell(row, m.stock_unit);
        let cat_raw = cell(row, m.category);
        let mut err: Option<String> = None;
        let unit = normalize_unit(&unit_raw);
        let cat = normalize_category(&cat_raw);
        if name.is_empty() {
            err = Some("Name is empty".into());
        } else if unit.is_none() {
            err = Some(format!("Unknown unit \"{unit_raw}\" (use g, kg, oz, lb, ml, l, fl oz, gal or piece)"));
        } else if cat.is_none() {
            err = Some(format!("Unknown category \"{cat_raw}\" (use professional, consumable or retail)"));
        }
        let nums = (|| -> Result<_, String> {
            Ok((
                parse_dec(&cell(row, m.density_g_per_ml), "Density")?,
                parse_dec(&cell(row, m.reorder_point), "Reorder point")?,
                parse_dec(&cell(row, m.retail_price), "Retail price")?,
                parse_dec(&cell(row, m.opening_qty), "Opening quantity")?,
                parse_dec(&cell(row, m.opening_unit_cost), "Opening unit cost")?,
            ))
        })();
        let (density, reorder, retail, oq, oc) = match nums {
            Ok(v) => v,
            Err(e) => {
                err = err.or(Some(e));
                (None, None, None, None, None)
            }
        };
        if err.is_none() && oq.is_some_and(|q| q > Decimal::ZERO) && oc.is_none() {
            err = Some("Opening quantity needs an opening unit cost".into());
        }
        if err.is_none() && [density, reorder, retail, oq, oc].iter().flatten().any(|d| *d < Decimal::ZERO) {
            err = Some("Numbers can't be negative".into());
        }
        let key = if !sku.is_empty() { format!("sku:{}", sku.to_lowercase()) } else { format!("name:{}|{}", name.to_lowercase(), brand.to_lowercase()) };
        let (status, message) = if let Some(e) = err {
            ("error", Some(e))
        } else if (!sku.is_empty() && skus.contains(&sku.to_lowercase())) || (!barcode.is_empty() && barcodes.contains(&barcode)) {
            ("duplicate", Some("A product with this SKU or barcode already exists; it will be skipped".to_string()))
        } else if sku.is_empty() && names.contains(&(name.to_lowercase(), brand.to_lowercase())) {
            ("duplicate", Some("A product with this name and brand already exists; it will be skipped".to_string()))
        } else if let Some(first) = seen.get(&key) {
            ("duplicate", Some(format!("Same product as line {first} in this file; it will be skipped")))
        } else {
            seen.insert(key, line);
            ("new", None)
        };
        let p = (status == "new").then(|| Parsed {
            input: ProductInput {
                id: None,
                name: name.clone(),
                brand: brand.clone(),
                category: cat.unwrap_or("professional").into(),
                subcategory: cell(row, m.subcategory),
                sku: sku.clone(),
                barcode,
                supplier_id: None,
                stock_unit: unit.unwrap_or("piece").into(),
                density_g_per_ml: density,
                default_storage_id: None,
                reorder_point: reorder,
                reorder_qty: None,
                retail_price: retail,
                notes: String::new(),
                custom_units: vec![],
            },
            supplier: cell(row, m.supplier),
            opening_qty: oq,
            opening_unit_cost: oc,
        });
        out.push(ImportRow {
            line,
            status: status.into(),
            message,
            name,
            brand,
            category: cat.unwrap_or("").into(),
            sku,
            stock_unit: unit.map(|u| units::unit(u).map(|d| d.label).unwrap_or(u)).unwrap_or("").into(),
            opening_qty: oq,
            opening_unit_cost: oc,
        });
        parsed.push(p);
    }
    Ok((out, parsed))
}

pub fn preview_products(conn: &Connection, table: &CsvTable, m: &ProductMapping) -> AppResult<ImportPreview> {
    let (rows, _) = parse_rows(conn, table, m)?;
    let count = |s: &str| rows.iter().filter(|r| r.status == s).count();
    Ok(ImportPreview { new_count: count("new"), duplicate_count: count("duplicate"), error_count: count("error"), rows })
}

/// Create the "new" rows (inside the caller's transaction). Returns how many were created.
pub fn import_products(conn: &Connection, table: &CsvTable, m: &ProductMapping, opening_date: &str) -> AppResult<usize> {
    let (rows, parsed) = parse_rows(conn, table, m)?;
    let mut suppliers: HashMap<String, i64> = inventory::suppliers(conn)?.into_iter().map(|s| (s.name.to_lowercase(), s.id.unwrap())).collect();
    let mut opening = Vec::new();
    let mut n = 0;
    for (row, p) in rows.iter().zip(parsed) {
        let Some(mut p) = p else { continue };
        if !p.supplier.is_empty() {
            let id = match suppliers.get(&p.supplier.to_lowercase()) {
                Some(id) => *id,
                None => {
                    let id = inventory::save_supplier(
                        conn,
                        &inventory::Supplier { id: None, name: p.supplier.clone(), contact: String::new(), phone: String::new(), email: String::new(), website: String::new(), notes: String::new(), archived: false },
                    )?;
                    suppliers.insert(p.supplier.to_lowercase(), id);
                    id
                }
            };
            p.input.supplier_id = Some(id);
        }
        let id = inventory::save_product(conn, &p.input).map_err(|e| AppError::msg(format!("Line {}: {e}", row.line)))?;
        if let (Some(q), Some(c)) = (p.opening_qty.filter(|q| *q > Decimal::ZERO), p.opening_unit_cost) {
            opening.push(PurchaseLineInput {
                product_id: id,
                package_count: Decimal::ONE,
                contents_per_package: q,
                unit: p.input.stock_unit.clone(),
                line_price: crate::domain::money::round_money(q * c),
                storage_id: None,
                lot_code: String::new(),
                expires_on: None,
            });
        }
        n += 1;
    }
    if !opening.is_empty() {
        inventory::receive(
            conn,
            &PurchaseInput {
                kind: "opening_balance".into(),
                supplier_id: None,
                purchase_date: opening_date.into(),
                invoice_ref: "CSV import".into(),
                discount: Decimal::ZERO,
                shipping: Decimal::ZERO,
                nonrecoverable_tax: Decimal::ZERO,
                attachment_id: None,
                notes: "Opening stock from CSV import".into(),
                lines: opening,
            },
        )?;
    }
    crate::db::audit(conn, "product", None, "import", &format!("Imported {n} products from CSV"), None)?;
    Ok(n)
}

pub fn export_products(conn: &Connection, path: &Path) -> AppResult<usize> {
    let rows = inventory::products(conn, true)?;
    write(
        path,
        &["name", "brand", "category", "subcategory", "sku", "barcode", "supplier", "stock_unit", "density_g_per_ml", "on_hand", "avg_cost_per_unit", "value", "reorder_point", "retail_price", "archived"],
        rows.into_iter().map(|p| {
            vec![
                p.name,
                p.brand,
                p.category,
                p.subcategory,
                p.sku,
                p.barcode,
                p.supplier_name.unwrap_or_default(),
                p.stock_unit,
                p.density_g_per_ml.map(|d| d.to_string()).unwrap_or_default(),
                p.on_hand.normalize().to_string(),
                p.avg_cost.map(|d| d.round_dp(6).normalize().to_string()).unwrap_or_default(),
                p.value.round_dp(2).to_string(),
                p.reorder_point.map(|d| d.normalize().to_string()).unwrap_or_default(),
                p.retail_price.map(|d| d.to_string()).unwrap_or_default(),
                p.archived.to_string(),
            ]
        }),
    )
}

pub fn export_ledger(conn: &Connection, path: &Path, f: &inventory::LedgerFilter) -> AppResult<usize> {
    let rows = inventory::ledger(conn, &inventory::LedgerFilter { limit: Some(f.limit.unwrap_or(1_000_000)), product_id: f.product_id, kind: f.kind.clone(), from: f.from.clone(), to: f.to.clone() })?;
    write(
        path,
        &["entry_id", "date", "recorded_at", "product", "storage", "kind", "qty", "unit", "value", "qty_after", "value_after", "source", "source_id", "reverses_entry", "note"],
        rows.into_iter().rev().map(|r| {
            vec![
                r.id.to_string(),
                r.occurred_on,
                r.recorded_at,
                r.product_name,
                r.storage_name.unwrap_or_default(),
                r.kind,
                r.qty.round_dp(6).normalize().to_string(),
                r.stock_unit,
                r.value.round_dp(6).normalize().to_string(),
                r.qty_after.round_dp(6).normalize().to_string(),
                r.value_after.round_dp(6).normalize().to_string(),
                r.source.unwrap_or_default(),
                r.source_id.map(|s| s.to_string()).unwrap_or_default(),
                r.reversal_of.map(|s| s.to_string()).unwrap_or_default(),
                r.note,
            ]
        }),
    )
}

/// Every table to `<table>.csv` inside a zip, with a README. Values are written exactly as stored
/// (decimals as text). API keys aren't in the database, so they can't leak into an export.
pub fn export_all(conn: &Connection, path: &Path) -> AppResult<usize> {
    use std::io::Write;
    let tables: Vec<String> = {
        let mut st = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> 'provider_cache' ORDER BY name")?;
        let rows = st.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let tmp = path.with_extension("zip.partial");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let zerr = |e: zip::result::ZipError| AppError::Other(e.to_string());
    z.start_file("README.txt", opts).map_err(zerr)?;
    z.write_all(
        format!(
            "Salon Resource Manager portable export\nCreated {}\nOne CSV per table. Amounts and quantities are exact decimals; quantities are in each product's base unit (g, ml or piece).\nThis is for reading elsewhere; to move data between computers use a backup instead.\n",
            crate::db::now_utc()
        )
        .as_bytes(),
    )?;
    for t in &tables {
        let mut st = conn.prepare(&format!("SELECT * FROM \"{t}\""))?;
        let cols: Vec<String> = st.column_names().iter().map(|c| c.to_string()).collect();
        let mut w = csv::Writer::from_writer(Vec::new());
        w.write_record(&cols).map_err(|e| AppError::Other(e.to_string()))?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let rec: Vec<String> = (0..cols.len())
                .map(|i| match r.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => String::new(),
                    Ok(rusqlite::types::ValueRef::Integer(v)) => v.to_string(),
                    Ok(rusqlite::types::ValueRef::Real(v)) => v.to_string(),
                    Ok(rusqlite::types::ValueRef::Text(v)) => sanitize(&String::from_utf8_lossy(v)),
                    Ok(rusqlite::types::ValueRef::Blob(_)) => "[binary]".into(),
                    Err(_) => String::new(),
                })
                .collect();
            w.write_record(&rec).map_err(|e| AppError::Other(e.to_string()))?;
        }
        z.start_file(format!("{t}.csv"), opts).map_err(zerr)?;
        z.write_all(&w.into_inner().map_err(|e| AppError::Other(e.to_string()))?)?;
    }
    z.finish().map_err(zerr)?;
    std::fs::rename(tmp, path)?;
    Ok(tables.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use rust_decimal_macros::dec;

    #[test]
    fn import_preview_validates_and_dedupes() {
        let conn = open_memory();
        let t = CsvTable {
            headers: vec!["Name".into(), "Brand".into(), "Unit".into(), "SKU".into(), "Qty".into(), "Cost".into(), "Category".into()],
            rows: vec![
                vec!["Developer 20".into(), "Acme".into(), "fl oz".into(), "D20".into(), "64".into(), "0.75".into(), "".into()],
                vec!["Developer 20".into(), "Acme".into(), "fl oz".into(), "D20".into(), "".into(), "".into(), "".into()],
                vec!["Gloves".into(), "".into(), "box".into(), "".into(), "".into(), "".into(), "".into()],
                vec!["Shampoo".into(), "Acme".into(), "ml".into(), "".into(), "10".into(), "".into(), "retail".into()],
                vec!["Foil".into(), "".into(), "each".into(), "".into(), "".into(), "".into(), "disposable".into()],
            ],
        };
        let m = ProductMapping { name: Some(0), brand: Some(1), stock_unit: Some(2), sku: Some(3), opening_qty: Some(4), opening_unit_cost: Some(5), category: Some(6), ..Default::default() };
        let pv = preview_products(&conn, &t, &m).unwrap();
        let st: Vec<&str> = pv.rows.iter().map(|r| r.status.as_str()).collect();
        assert_eq!(st, vec!["new", "duplicate", "error", "error", "new"]);
        assert_eq!(import_products(&conn, &t, &m, "2026-10-01").unwrap(), 2);
        let dev = inventory::products(&conn, false).unwrap().into_iter().find(|p| p.sku == "D20").unwrap();
        assert_eq!(dev.on_hand, dec!(64));
        assert_eq!(dev.value, dec!(48));
        // importing again: everything is now a duplicate or error
        let again = preview_products(&conn, &t, &m).unwrap();
        assert_eq!(again.new_count, 0);
    }

    #[test]
    fn export_all_writes_every_table() {
        let conn = open_memory();
        let dir = std::env::temp_dir().join(format!("srm-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("all.zip");
        let n = export_all(&conn, &p).unwrap();
        let z = zip::ZipArchive::new(std::fs::File::open(&p).unwrap()).unwrap();
        assert_eq!(z.len(), n + 1);
        assert!(z.file_names().any(|f| f == "stock_ledger.csv"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn export_sanitizes_formulas() {
        assert_eq!(sanitize("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(sanitize("-5"), "-5");
        assert_eq!(sanitize("-cmd"), "'-cmd");
    }
}
