//! Products, suppliers, storage, purchases and the stock ledger. Every stock change goes through
//! `post`, which applies the moving-average rules in `domain::inventory` and appends ledger rows in
//! the caller's transaction.

use super::{audit, dec, now_utc, opt_dec};
use crate::domain::inventory::{Movement, Stock};
use crate::domain::money::round_money;
use crate::domain::units::{self, CustomUnit, Dimension, ProductUnits};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn valid_date(field: &str, s: &str) -> AppResult<()> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| AppError::invalid(field, "Use a date like 2026-10-04."))
}

fn money_input(field: &str, label: &str, d: Decimal) -> AppResult<Decimal> {
    crate::domain::money::non_negative(field, label, d)?;
    if d.scale() > 2 && d != d.round_dp(2) {
        return Err(AppError::invalid(field, format!("{label} must be in whole cents.")));
    }
    Ok(d)
}

// ---------------------------------------------------------------- suppliers & storage

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Supplier {
    pub id: Option<i64>,
    pub name: String,
    pub contact: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    pub notes: String,
    #[serde(default)]
    pub archived: bool,
}

pub fn suppliers(conn: &Connection) -> AppResult<Vec<Supplier>> {
    let mut st = conn.prepare("SELECT id, name, contact, phone, email, website, notes, archived_at FROM suppliers ORDER BY archived_at IS NOT NULL, name COLLATE NOCASE")?;
    let rows = st.query_map([], |r| {
        Ok(Supplier {
            id: r.get(0)?,
            name: r.get(1)?,
            contact: r.get(2)?,
            phone: r.get(3)?,
            email: r.get(4)?,
            website: r.get(5)?,
            notes: r.get(6)?,
            archived: r.get::<_, Option<String>>(7)?.is_some(),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn save_supplier(conn: &Connection, s: &Supplier) -> AppResult<i64> {
    if s.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Enter the supplier's name."));
    }
    let p = params![s.name.trim(), s.contact.trim(), s.phone.trim(), s.email.trim(), s.website.trim(), s.notes, s.id];
    let id = if s.id.is_some() {
        conn.execute("UPDATE suppliers SET name=?1, contact=?2, phone=?3, email=?4, website=?5, notes=?6 WHERE id=?7", p)?;
        s.id.unwrap()
    } else {
        conn.execute("INSERT INTO suppliers (name, contact, phone, email, website, notes) VALUES (?1,?2,?3,?4,?5,?6)", &p[..6])?;
        conn.last_insert_rowid()
    };
    audit(conn, "supplier", Some(id), if s.id.is_some() { "update" } else { "create" }, &format!("Supplier \"{}\" saved", s.name.trim()), None)?;
    Ok(id)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct StorageLocation {
    pub id: Option<i64>,
    pub name: String,
    pub location_id: Option<i64>,
    #[serde(default)]
    pub archived: bool,
}

pub fn storage_locations(conn: &Connection) -> AppResult<Vec<StorageLocation>> {
    let mut st = conn.prepare("SELECT id, name, location_id, archived_at FROM storage_locations ORDER BY archived_at IS NOT NULL, name COLLATE NOCASE")?;
    let rows = st.query_map([], |r| {
        Ok(StorageLocation { id: r.get(0)?, name: r.get(1)?, location_id: r.get(2)?, archived: r.get::<_, Option<String>>(3)?.is_some() })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn save_storage(conn: &Connection, s: &StorageLocation) -> AppResult<i64> {
    if s.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Name the storage spot, e.g. Back room or Color bar."));
    }
    let id = match s.id {
        Some(id) => {
            conn.execute("UPDATE storage_locations SET name=?1, location_id=?2 WHERE id=?3", params![s.name.trim(), s.location_id, id])?;
            id
        }
        None => {
            conn.execute("INSERT INTO storage_locations (name, location_id) VALUES (?1, ?2)", params![s.name.trim(), s.location_id])?;
            conn.last_insert_rowid()
        }
    };
    audit(conn, "storage_location", Some(id), "save", &format!("Storage \"{}\" saved", s.name.trim()), None)?;
    Ok(id)
}

// ---------------------------------------------------------------- products

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ProductInput {
    pub id: Option<i64>,
    pub name: String,
    pub brand: String,
    pub category: String,
    pub subcategory: String,
    pub sku: String,
    pub barcode: String,
    pub supplier_id: Option<i64>,
    pub stock_unit: String,
    pub density_g_per_ml: Option<Decimal>,
    pub default_storage_id: Option<i64>,
    /// In the stock unit
    pub reorder_point: Option<Decimal>,
    pub reorder_qty: Option<Decimal>,
    pub retail_price: Option<Decimal>,
    pub notes: String,
    pub custom_units: Vec<CustomUnit>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ProductRow {
    pub id: i64,
    pub name: String,
    pub brand: String,
    pub category: String,
    pub subcategory: String,
    pub sku: String,
    pub barcode: String,
    pub supplier_id: Option<i64>,
    pub supplier_name: Option<String>,
    pub dimension: Dimension,
    pub stock_unit: String,
    pub base_unit: String,
    pub density_g_per_ml: Option<Decimal>,
    pub default_storage_id: Option<i64>,
    pub storage_name: Option<String>,
    pub reorder_point: Option<Decimal>,
    pub reorder_qty: Option<Decimal>,
    pub retail_price: Option<Decimal>,
    /// On hand in the stock unit
    pub on_hand: Decimal,
    pub on_hand_base: Decimal,
    pub value: Decimal,
    /// Average cost per stock unit
    pub avg_cost: Option<Decimal>,
    pub low_stock: bool,
    pub negative: bool,
    pub custom_units: Vec<CustomUnit>,
    pub notes: String,
    pub archived: bool,
}

impl ProductRow {
    pub fn units(&self) -> ProductUnits {
        ProductUnits { dimension: Some(self.dimension), density_g_per_ml: self.density_g_per_ml, custom: self.custom_units.clone() }
    }
}

fn custom_units_all(conn: &Connection) -> AppResult<HashMap<i64, Vec<CustomUnit>>> {
    let mut st = conn.prepare("SELECT product_id, name, qty, unit FROM product_units ORDER BY name")?;
    let mut map: HashMap<i64, Vec<CustomUnit>> = HashMap::new();
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, CustomUnit { name: r.get(1)?, qty: dec(r, "qty")?, unit: r.get(3)? })))?;
    for row in rows {
        let (pid, cu) = row?;
        map.entry(pid).or_default().push(cu);
    }
    Ok(map)
}

const PRODUCT_SQL: &str = "SELECT p.id, p.name, p.brand, p.category, p.subcategory, p.sku, p.barcode, p.supplier_id, s.name AS supplier_name,
    p.dimension, p.stock_unit, p.density_g_per_ml, p.default_storage_id, sl.name AS storage_name, p.reorder_point, p.reorder_qty,
    p.retail_price, p.on_hand_qty, p.on_hand_value, p.notes, p.archived_at
    FROM products p LEFT JOIN suppliers s ON s.id = p.supplier_id LEFT JOIN storage_locations sl ON sl.id = p.default_storage_id";

fn product_from_row(r: &rusqlite::Row, custom: &HashMap<i64, Vec<CustomUnit>>) -> AppResult<ProductRow> {
    let id: i64 = r.get("id")?;
    let dimension = Dimension::parse(&r.get::<_, String>("dimension")?)?;
    let stock_unit: String = r.get("stock_unit")?;
    let pu = ProductUnits { dimension: Some(dimension), density_g_per_ml: opt_dec(r, "density_g_per_ml")?, custom: custom.get(&id).cloned().unwrap_or_default() };
    let on_hand_base = dec(r, "on_hand_qty")?;
    let value = dec(r, "on_hand_value")?;
    let unit_size = pu.to_base(Decimal::ONE, &stock_unit)?;
    let to_stock = |base: Decimal| base / unit_size;
    let rp_base = opt_dec(r, "reorder_point")?;
    let avg_cost = if on_hand_base > Decimal::ZERO { Some(value * unit_size / on_hand_base) } else { None };
    Ok(ProductRow {
        id,
        name: r.get("name")?,
        brand: r.get("brand")?,
        category: r.get("category")?,
        subcategory: r.get("subcategory")?,
        sku: r.get("sku")?,
        barcode: r.get("barcode")?,
        supplier_id: r.get("supplier_id")?,
        supplier_name: r.get("supplier_name")?,
        dimension,
        base_unit: dimension.base_unit().into(),
        density_g_per_ml: pu.density_g_per_ml,
        default_storage_id: r.get("default_storage_id")?,
        storage_name: r.get("storage_name")?,
        reorder_point: rp_base.map(to_stock),
        reorder_qty: opt_dec(r, "reorder_qty")?.map(to_stock),
        retail_price: opt_dec(r, "retail_price")?,
        on_hand: to_stock(on_hand_base),
        on_hand_base,
        value,
        avg_cost,
        low_stock: rp_base.is_some_and(|rp| on_hand_base <= rp),
        negative: on_hand_base < Decimal::ZERO,
        custom_units: pu.custom,
        stock_unit,
        notes: r.get("notes")?,
        archived: r.get::<_, Option<String>>("archived_at")?.is_some(),
    })
}

pub fn products(conn: &Connection, include_archived: bool) -> AppResult<Vec<ProductRow>> {
    let custom = custom_units_all(conn)?;
    let mut st = conn.prepare(&format!("{PRODUCT_SQL} WHERE (?1 OR p.archived_at IS NULL) ORDER BY p.name COLLATE NOCASE, p.brand"))?;
    let mut rows = st.query([include_archived])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(product_from_row(r, &custom)?);
    }
    Ok(out)
}

pub fn product(conn: &Connection, id: i64) -> AppResult<ProductRow> {
    let custom = custom_units_all(conn)?;
    let mut st = conn.prepare(&format!("{PRODUCT_SQL} WHERE p.id = ?1"))?;
    let mut rows = st.query([id])?;
    match rows.next()? {
        Some(r) => product_from_row(r, &custom),
        None => Err(AppError::NotFound(format!("Product {id} not found."))),
    }
}

pub fn product_units(conn: &Connection, id: i64) -> AppResult<ProductUnits> {
    Ok(product(conn, id)?.units())
}

fn ledger_count(conn: &Connection, product_id: i64) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM stock_ledger WHERE product_id = ?1", [product_id], |r| r.get(0))?)
}

pub fn save_product(conn: &Connection, p: &ProductInput) -> AppResult<i64> {
    let name = p.name.trim();
    if name.is_empty() {
        return Err(AppError::invalid("name", "Enter a product name."));
    }
    if !matches!(p.category.as_str(), "professional" | "consumable" | "retail") {
        return Err(AppError::invalid("category", "Choose professional supply, disposable consumable or retail product."));
    }
    let u = units::unit(&p.stock_unit).map_err(|_| AppError::invalid("stock_unit", "Choose a standard stock unit."))?;
    if let Some(rho) = p.density_g_per_ml {
        if rho <= Decimal::ZERO {
            return Err(AppError::invalid("density_g_per_ml", "Density must be more than zero, or leave it blank."));
        }
    }
    if p.category == "retail" {
        if let Some(rp) = p.retail_price {
            money_input("retail_price", "Retail price", rp)?;
        }
    }
    let pu = ProductUnits { dimension: Some(u.dimension), density_g_per_ml: p.density_g_per_ml, custom: vec![] };
    let mut seen = std::collections::HashSet::new();
    for c in &p.custom_units {
        units::validate_custom(&pu, c)?;
        if !seen.insert(c.name.trim().to_lowercase()) {
            return Err(AppError::invalid("custom_units", format!("The unit \"{}\" is listed twice.", c.name.trim())));
        }
    }
    let unit_size = pu.to_base(Decimal::ONE, &p.stock_unit)?;
    for (f, v) in [("reorder_point", p.reorder_point), ("reorder_qty", p.reorder_qty)] {
        if v.is_some_and(|v| v < Decimal::ZERO) {
            return Err(AppError::invalid(f, "Can't be negative."));
        }
    }
    let rp = p.reorder_point.map(|v| (v * unit_size).to_string());
    let rq = p.reorder_qty.map(|v| (v * unit_size).to_string());
    let dup = |col: &str, val: &str| -> AppResult<()> {
        if val.trim().is_empty() {
            return Ok(());
        }
        let other: Option<String> = conn
            .query_row(&format!("SELECT name FROM products WHERE {col} = ?1 AND id IS NOT ?2"), params![val.trim(), p.id], |r| r.get(0))
            .optional()?;
        match other {
            Some(n) => Err(AppError::invalid(col, format!("Another product (\"{n}\") already uses this {}.", if col == "sku" { "SKU" } else { "barcode" }))),
            None => Ok(()),
        }
    };
    dup("sku", &p.sku)?;
    dup("barcode", &p.barcode)?;

    let id = match p.id {
        Some(id) => {
            let cur = product(conn, id)?;
            if cur.dimension != u.dimension && ledger_count(conn, id)? > 0 {
                return Err(AppError::invalid(
                    "stock_unit",
                    format!("This product already has stock history measured by {}. Pick a {} unit.", cur.dimension.as_str(), cur.dimension.as_str()),
                ));
            }
            conn.execute(
                "UPDATE products SET name=?1, brand=?2, category=?3, subcategory=?4, sku=?5, barcode=?6, supplier_id=?7, dimension=?8, stock_unit=?9,
                 density_g_per_ml=?10, default_storage_id=?11, reorder_point=?12, reorder_qty=?13, retail_price=?14, notes=?15, updated_at=?16 WHERE id=?17",
                params![
                    name, p.brand.trim(), p.category, p.subcategory.trim(), p.sku.trim(), p.barcode.trim(), p.supplier_id, u.dimension.as_str(), p.stock_unit,
                    p.density_g_per_ml.map(|d| d.to_string()), p.default_storage_id, rp, rq, p.retail_price.map(|d| d.to_string()), p.notes, now_utc(), id
                ],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO products (name, brand, category, subcategory, sku, barcode, supplier_id, dimension, stock_unit, density_g_per_ml,
                 default_storage_id, reorder_point, reorder_qty, retail_price, notes) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                params![
                    name, p.brand.trim(), p.category, p.subcategory.trim(), p.sku.trim(), p.barcode.trim(), p.supplier_id, u.dimension.as_str(), p.stock_unit,
                    p.density_g_per_ml.map(|d| d.to_string()), p.default_storage_id, rp, rq, p.retail_price.map(|d| d.to_string()), p.notes
                ],
            )?;
            conn.last_insert_rowid()
        }
    };
    // Custom units: replace the set (they are definitions, not history; usage rows store base quantities).
    conn.execute("DELETE FROM product_units WHERE product_id = ?1", [id])?;
    for c in &p.custom_units {
        conn.execute("INSERT INTO product_units (product_id, name, qty, unit) VALUES (?1, ?2, ?3, ?4)", params![id, c.name.trim(), c.qty.to_string(), c.unit])?;
    }
    audit(conn, "product", Some(id), if p.id.is_some() { "update" } else { "create" }, &format!("Product \"{name}\" saved"), Some(serde_json::to_value(p)?))?;
    Ok(id)
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct BulkPatch {
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub supplier_id: Option<Option<i64>>,
    pub default_storage_id: Option<Option<i64>>,
    /// In each product's stock unit
    pub reorder_point: Option<Option<Decimal>>,
    pub archived: Option<bool>,
}

/// Apply the same change to many products in one transaction.
pub fn bulk_update(conn: &Connection, ids: &[i64], patch: &BulkPatch) -> AppResult<usize> {
    if ids.is_empty() {
        return Err(AppError::msg("Select at least one product."));
    }
    if let Some(c) = &patch.category {
        if !matches!(c.as_str(), "professional" | "consumable" | "retail") {
            return Err(AppError::invalid("category", "Unknown category."));
        }
    }
    for &id in ids {
        let p = product(conn, id)?;
        if let Some(c) = &patch.category {
            conn.execute("UPDATE products SET category = ?1 WHERE id = ?2", params![c, id])?;
        }
        if let Some(s) = &patch.subcategory {
            conn.execute("UPDATE products SET subcategory = ?1 WHERE id = ?2", params![s.trim(), id])?;
        }
        if let Some(s) = patch.supplier_id {
            conn.execute("UPDATE products SET supplier_id = ?1 WHERE id = ?2", params![s, id])?;
        }
        if let Some(s) = patch.default_storage_id {
            conn.execute("UPDATE products SET default_storage_id = ?1 WHERE id = ?2", params![s, id])?;
        }
        if let Some(rp) = patch.reorder_point {
            if rp.is_some_and(|v| v < Decimal::ZERO) {
                return Err(AppError::invalid("reorder_point", "Can't be negative."));
            }
            let base = rp.map(|v| -> AppResult<String> { Ok((v * p.units().to_base(Decimal::ONE, &p.stock_unit)?).to_string()) }).transpose()?;
            conn.execute("UPDATE products SET reorder_point = ?1 WHERE id = ?2", params![base, id])?;
        }
        if let Some(a) = patch.archived {
            conn.execute("UPDATE products SET archived_at = ?1 WHERE id = ?2", params![a.then(now_utc), id])?;
        }
        conn.execute("UPDATE products SET updated_at = ?1 WHERE id = ?2", params![now_utc(), id])?;
    }
    audit(conn, "product", None, "bulk_update", &format!("Bulk edit of {} products", ids.len()), Some(serde_json::json!({ "ids": ids })))?;
    Ok(ids.len())
}

// ---------------------------------------------------------------- ledger posting

pub struct PostCtx<'a> {
    pub occurred_on: &'a str,
    pub storage_id: Option<i64>,
    pub source: Option<(&'a str, i64)>,
    pub note: &'a str,
}

pub enum Op {
    Receive { kind: &'static str, qty: Decimal, cost: Decimal },
    Issue { kind: &'static str, qty: Decimal },
    /// Count to an exact on-hand quantity
    AdjustTo { counted: Decimal, unit_cost: Option<Decimal> },
    Reverse { entry_id: i64 },
}

#[derive(Serialize, Debug, Clone)]
pub struct Posted {
    pub ledger_id: i64,
    pub qty: Decimal,
    /// Signed value moved (negative for issues); for issues, -value is the cost assigned.
    pub value: Decimal,
    pub went_negative: bool,
}

fn load_stock(conn: &Connection, product_id: i64) -> AppResult<Stock> {
    conn.query_row("SELECT on_hand_qty, on_hand_value, last_unit_cost FROM products WHERE id = ?1", [product_id], |r| {
        Ok(Stock { qty: dec(r, "on_hand_qty")?, value: dec(r, "on_hand_value")?, last_unit_cost: opt_dec(r, "last_unit_cost")? })
    })
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("Product {product_id} not found.")))
}

#[allow(clippy::too_many_arguments)]
fn insert_ledger(
    conn: &Connection,
    product_id: i64,
    ctx: &PostCtx,
    kind: &str,
    qty: Decimal,
    value: Decimal,
    qty_after: Decimal,
    value_after: Decimal,
    source: Option<(&str, i64)>,
    reversal_of: Option<i64>,
) -> AppResult<i64> {
    let res = conn.execute(
        "INSERT INTO stock_ledger (product_id, storage_id, occurred_on, kind, qty, value, qty_after, value_after, source, source_id, reversal_of, note)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            product_id,
            ctx.storage_id,
            ctx.occurred_on,
            kind,
            qty.normalize().to_string(),
            value.to_string(),
            qty_after.normalize().to_string(),
            value_after.to_string(),
            source.map(|s| s.0),
            source.map(|s| s.1),
            reversal_of,
            ctx.note
        ],
    );
    match res {
        Ok(_) => Ok(conn.last_insert_rowid()),
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
            Err(AppError::Conflict("This stock movement was already recorded (duplicate submission ignored).".into()))
        }
        Err(e) => Err(e.into()),
    }
}

/// Apply one stock operation: ledger row(s) + product totals. Run inside a transaction.
pub fn post(conn: &Connection, product_id: i64, op: Op, ctx: PostCtx) -> AppResult<Posted> {
    valid_date("occurred_on", ctx.occurred_on)?;
    let stock = load_stock(conn, product_id)?;
    let (kind, qty, m, reversal_of): (&str, Decimal, Movement, Option<i64>) = match op {
        Op::Receive { kind, qty, cost } => (kind, qty, stock.receive(qty, cost)?, None),
        Op::Issue { kind, qty } => (kind, -qty, stock.issue(qty)?, None),
        Op::AdjustTo { counted, unit_cost } => {
            let m = stock.adjust_to(counted, unit_cost)?;
            ("adjustment", counted - stock.qty, m, None)
        }
        Op::Reverse { entry_id } => {
            let (pid, kind, q, v, already): (i64, String, String, String, Option<i64>) = conn
                .query_row(
                    "SELECT l.product_id, l.kind, l.qty, l.value, (SELECT id FROM stock_ledger WHERE reversal_of = l.id) FROM stock_ledger l WHERE l.id = ?1",
                    [entry_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()?
                .ok_or_else(|| AppError::NotFound("Stock entry not found.".into()))?;
            if pid != product_id {
                return Err(AppError::msg("Entry belongs to a different product."));
            }
            if already.is_some() {
                return Err(AppError::Conflict("This entry has already been reversed.".into()));
            }
            if kind == "reversal" || kind == "revaluation" {
                return Err(AppError::msg("Corrections can't themselves be reversed. Record a new movement instead."));
            }
            let q: Decimal = q.parse().map_err(|_| AppError::Other("Bad ledger quantity".into()))?;
            let v: Decimal = v.parse().map_err(|_| AppError::Other("Bad ledger value".into()))?;
            ("reversal", -q, stock.reverse(q, v), Some(entry_id))
        }
    };
    let main_value_after = m.after.value - m.revaluation;
    let id = insert_ledger(conn, product_id, &ctx, kind, qty, m.value, m.after.qty, main_value_after, ctx.source, reversal_of)?;
    if !m.revaluation.is_zero() {
        insert_ledger(
            conn,
            product_id,
            &PostCtx { note: "Value correction so remaining stock is valued at the latest cost", ..ctx },
            "revaluation",
            Decimal::ZERO,
            m.revaluation,
            m.after.qty,
            m.after.value,
            Some(("ledger", id)),
            None,
        )?;
    }
    conn.execute(
        "UPDATE products SET on_hand_qty = ?1, on_hand_value = ?2, last_unit_cost = ?3, updated_at = ?4 WHERE id = ?5",
        params![m.after.qty.normalize().to_string(), m.after.value.to_string(), m.after.last_unit_cost.map(|d| d.to_string()), now_utc(), product_id],
    )?;
    Ok(Posted { ledger_id: id, qty, value: m.value, went_negative: m.went_negative })
}

/// Move stock between storage spots. Value doesn't change.
pub fn transfer(conn: &Connection, product_id: i64, qty_base: Decimal, from: Option<i64>, to: Option<i64>, date: &str, note: &str) -> AppResult<()> {
    valid_date("occurred_on", date)?;
    if qty_base <= Decimal::ZERO {
        return Err(AppError::invalid("qty", "Quantity must be more than zero."));
    }
    if from == to {
        return Err(AppError::invalid("to_storage_id", "Choose a different destination."));
    }
    let s = load_stock(conn, product_id)?;
    let out = insert_ledger(conn, product_id, &PostCtx { occurred_on: date, storage_id: from, source: None, note }, "transfer_out", -qty_base, Decimal::ZERO, s.qty, s.value, None, None)?;
    insert_ledger(conn, product_id, &PostCtx { occurred_on: date, storage_id: to, source: None, note }, "transfer_in", qty_base, Decimal::ZERO, s.qty, s.value, Some(("transfer", out)), None)?;
    Ok(())
}

// ---------------------------------------------------------------- manual movements

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct MovementInput {
    pub product_id: i64,
    /// waste | adjustment | transfer | supplier_return
    pub kind: String,
    /// For waste/transfer/supplier_return: quantity moved. For adjustment: the counted quantity.
    pub qty: Decimal,
    pub unit: String,
    pub storage_id: Option<i64>,
    pub to_storage_id: Option<i64>,
    /// Adjustment gains with no average cost yet: cost per `unit`
    pub unit_cost: Option<Decimal>,
    pub occurred_on: String,
    pub note: String,
}

pub fn record_movement(conn: &Connection, m: &MovementInput) -> AppResult<Option<Posted>> {
    let pu = product_units(conn, m.product_id)?;
    if m.qty < Decimal::ZERO {
        return Err(AppError::invalid("qty", "Quantity can't be negative."));
    }
    let q = pu.to_base(m.qty, &m.unit).map_err(|e| AppError::invalid("unit", e.to_string()))?;
    let ctx = PostCtx { occurred_on: &m.occurred_on, storage_id: m.storage_id, source: None, note: &m.note };
    let name = product(conn, m.product_id)?.name;
    let out = match m.kind.as_str() {
        "waste" | "supplier_return" => {
            if q.is_zero() {
                return Err(AppError::invalid("qty", "Quantity must be more than zero."));
            }
            let kind = if m.kind == "waste" { "waste" } else { "supplier_return" };
            Some(post(conn, m.product_id, Op::Issue { kind, qty: q }, ctx)?)
        }
        "adjustment" => {
            let unit_cost_base = match m.unit_cost {
                Some(c) => {
                    crate::domain::money::non_negative("unit_cost", "Cost", c)?;
                    Some(c / pu.to_base(Decimal::ONE, &m.unit)?)
                }
                None => None,
            };
            Some(post(conn, m.product_id, Op::AdjustTo { counted: q, unit_cost: unit_cost_base }, ctx)?)
        }
        "transfer" => {
            transfer(conn, m.product_id, q, m.storage_id, m.to_storage_id, &m.occurred_on, &m.note)?;
            None
        }
        _ => return Err(AppError::invalid("kind", "Unknown movement type.")),
    };
    audit(conn, "stock", Some(m.product_id), &m.kind, &format!("{} recorded for \"{name}\"", m.kind.replace('_', " ")), Some(serde_json::json!({"qty": m.qty, "unit": m.unit, "note": m.note})))?;
    Ok(out)
}

/// Reverse a manual movement (waste, adjustment, supplier return, transfer).
pub fn reverse_entry(conn: &Connection, entry_id: i64, date: &str, note: &str) -> AppResult<()> {
    let (pid, kind, source, source_id): (i64, String, Option<String>, Option<i64>) = conn
        .query_row("SELECT product_id, kind, source, source_id FROM stock_ledger WHERE id = ?1", [entry_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()?
        .ok_or_else(|| AppError::NotFound("Stock entry not found.".into()))?;
    match kind.as_str() {
        "waste" | "adjustment" | "supplier_return" => {
            post(conn, pid, Op::Reverse { entry_id }, PostCtx { occurred_on: date, storage_id: None, source: None, note })?;
        }
        "transfer_out" | "transfer_in" => {
            let (out_id, in_id) = if kind == "transfer_out" {
                let in_id: i64 = conn.query_row("SELECT id FROM stock_ledger WHERE source='transfer' AND source_id=?1", [entry_id], |r| r.get(0))?;
                (entry_id, in_id)
            } else {
                (source_id.filter(|_| source.as_deref() == Some("transfer")).ok_or_else(|| AppError::Other("Transfer pair missing".into()))?, entry_id)
            };
            let row = |id: i64| -> AppResult<(Option<i64>, String)> { Ok(conn.query_row("SELECT storage_id, qty FROM stock_ledger WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?) };
            let (from, qty) = row(in_id)?;
            let (to, _) = row(out_id)?;
            let reversed: i64 = conn.query_row("SELECT COUNT(*) FROM stock_ledger WHERE reversal_of IN (?1, ?2)", [out_id, in_id], |r| r.get(0))?;
            if reversed > 0 {
                return Err(AppError::Conflict("This transfer has already been reversed.".into()));
            }
            let s = load_stock(conn, pid)?;
            let q: Decimal = qty.parse().map_err(|_| AppError::Other("Bad qty".into()))?;
            let ctx_from = PostCtx { occurred_on: date, storage_id: from, source: None, note };
            insert_ledger(conn, pid, &ctx_from, "reversal", -q, Decimal::ZERO, s.qty, s.value, None, Some(in_id))?;
            let ctx_to = PostCtx { occurred_on: date, storage_id: to, source: None, note };
            insert_ledger(conn, pid, &ctx_to, "reversal", q, Decimal::ZERO, s.qty, s.value, None, Some(out_id))?;
        }
        "purchase" | "opening" => return Err(AppError::msg("Reverse the whole purchase from the Purchases tab instead.")),
        "service_use" | "retail_sale" | "customer_return" => {
            return Err(AppError::msg("Sale-related stock is corrected by voiding or refunding the sale."));
        }
        _ => return Err(AppError::msg("This entry can't be reversed.")),
    }
    audit(conn, "stock", Some(pid), "reverse", &format!("Stock entry {entry_id} reversed"), Some(serde_json::json!({ "note": note })))?;
    Ok(())
}

#[derive(Serialize, Debug, Clone)]
pub struct LedgerRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub storage_id: Option<i64>,
    pub storage_name: Option<String>,
    pub occurred_on: String,
    pub recorded_at: String,
    pub kind: String,
    pub qty_base: Decimal,
    /// In the product's stock unit
    pub qty: Decimal,
    pub stock_unit: String,
    pub value: Decimal,
    pub qty_after: Decimal,
    pub value_after: Decimal,
    pub source: Option<String>,
    pub source_id: Option<i64>,
    pub reversal_of: Option<i64>,
    pub reversed_by: Option<i64>,
    pub note: String,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct LedgerFilter {
    pub product_id: Option<i64>,
    pub kind: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
}

pub fn ledger(conn: &Connection, f: &LedgerFilter) -> AppResult<Vec<LedgerRow>> {
    let custom = custom_units_all(conn)?;
    let mut st = conn.prepare(
        "SELECT l.id, l.product_id, p.name, l.storage_id, s.name, l.occurred_on, l.recorded_at, l.kind, l.qty, l.value, l.qty_after, l.value_after,
                l.source, l.source_id, l.reversal_of, (SELECT r.id FROM stock_ledger r WHERE r.reversal_of = l.id), l.note,
                p.dimension, p.stock_unit, p.density_g_per_ml
         FROM stock_ledger l JOIN products p ON p.id = l.product_id LEFT JOIN storage_locations s ON s.id = l.storage_id
         WHERE (?1 IS NULL OR l.product_id = ?1) AND (?2 IS NULL OR l.kind = ?2) AND (?3 IS NULL OR l.occurred_on >= ?3) AND (?4 IS NULL OR l.occurred_on <= ?4)
         ORDER BY l.id DESC LIMIT ?5",
    )?;
    let mut rows = st.query(params![f.product_id, f.kind, f.from, f.to, f.limit.unwrap_or(500).clamp(1, 20000)])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let pid: i64 = r.get(1)?;
        let pu = ProductUnits {
            dimension: Some(Dimension::parse(&r.get::<_, String>(17)?)?),
            density_g_per_ml: opt_dec(r, "density_g_per_ml")?,
            custom: custom.get(&pid).cloned().unwrap_or_default(),
        };
        let stock_unit: String = r.get(18)?;
        let size = pu.to_base(Decimal::ONE, &stock_unit)?;
        let qty_base = dec(r, "qty")?;
        out.push(LedgerRow {
            id: r.get(0)?,
            product_id: pid,
            product_name: r.get(2)?,
            storage_id: r.get(3)?,
            storage_name: r.get(4)?,
            occurred_on: r.get(5)?,
            recorded_at: r.get(6)?,
            kind: r.get(7)?,
            qty_base,
            qty: qty_base / size,
            value: dec(r, "value")?,
            qty_after: dec(r, "qty_after")? / size,
            value_after: dec(r, "value_after")?,
            source: r.get(12)?,
            source_id: r.get(13)?,
            reversal_of: r.get(14)?,
            reversed_by: r.get(15)?,
            note: r.get(16)?,
            stock_unit,
        });
    }
    Ok(out)
}

#[derive(Serialize, Debug, Clone)]
pub struct StorageQty {
    pub storage_id: Option<i64>,
    pub storage_name: Option<String>,
    /// In the stock unit
    pub qty: Decimal,
}

pub fn stock_by_storage(conn: &Connection, product_id: i64) -> AppResult<Vec<StorageQty>> {
    let p = product(conn, product_id)?;
    let size = p.units().to_base(Decimal::ONE, &p.stock_unit)?;
    let mut st = conn.prepare("SELECT l.storage_id, s.name, l.qty FROM stock_ledger l LEFT JOIN storage_locations s ON s.id = l.storage_id WHERE l.product_id = ?1")?;
    let mut sums: Vec<StorageQty> = Vec::new();
    let mut rows = st.query([product_id])?;
    while let Some(r) = rows.next()? {
        let sid: Option<i64> = r.get(0)?;
        let q = dec(r, "qty")? / size;
        match sums.iter_mut().find(|s| s.storage_id == sid) {
            Some(s) => s.qty += q,
            None => sums.push(StorageQty { storage_id: sid, storage_name: r.get(1)?, qty: q }),
        }
    }
    sums.retain(|s| !s.qty.is_zero());
    Ok(sums)
}

// ---------------------------------------------------------------- purchases

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PurchaseLineInput {
    pub product_id: i64,
    pub package_count: Decimal,
    pub contents_per_package: Decimal,
    pub unit: String,
    pub line_price: Decimal,
    pub storage_id: Option<i64>,
    pub lot_code: String,
    pub expires_on: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PurchaseInput {
    /// "purchase" or "opening_balance" (stock you already had; not cash spending)
    pub kind: String,
    pub supplier_id: Option<i64>,
    pub purchase_date: String,
    pub invoice_ref: String,
    pub discount: Decimal,
    pub shipping: Decimal,
    pub nonrecoverable_tax: Decimal,
    pub attachment_id: Option<i64>,
    pub notes: String,
    pub lines: Vec<PurchaseLineInput>,
}

#[derive(Serialize, Debug, Clone)]
pub struct LinePreview {
    pub qty_base: Decimal,
    /// Quantity in the product's stock unit
    pub qty_stock: Decimal,
    pub stock_unit: String,
    pub landed_cost: Decimal,
    /// Landed cost per stock unit
    pub unit_cost: Decimal,
}

#[derive(Serialize, Debug, Clone)]
pub struct PurchasePreview {
    pub subtotal: Decimal,
    pub total: Decimal,
    pub lines: Vec<LinePreview>,
}

pub fn preview_purchase(conn: &Connection, p: &PurchaseInput) -> AppResult<PurchasePreview> {
    if p.lines.is_empty() {
        return Err(AppError::invalid("lines", "Add at least one product."));
    }
    money_input("discount", "Discount", p.discount)?;
    money_input("shipping", "Shipping", p.shipping)?;
    money_input("nonrecoverable_tax", "Purchase tax", p.nonrecoverable_tax)?;
    let mut subtotal = Decimal::ZERO;
    let mut qtys = Vec::new();
    for (i, l) in p.lines.iter().enumerate() {
        let f = |name: &str| format!("lines.{i}.{name}");
        if l.package_count <= Decimal::ZERO {
            return Err(AppError::invalid(&f("package_count"), "Package count must be more than zero."));
        }
        if l.contents_per_package <= Decimal::ZERO {
            return Err(AppError::invalid(&f("contents_per_package"), "Contents per package must be more than zero."));
        }
        money_input(&f("line_price"), "Line price", l.line_price)?;
        if let Some(e) = &l.expires_on {
            valid_date(&f("expires_on"), e)?;
        }
        let prod = product(conn, l.product_id)?;
        let pu = prod.units();
        let qb = pu.to_base(l.package_count * l.contents_per_package, &l.unit).map_err(|e| AppError::invalid(&f("unit"), e.to_string()))?;
        qtys.push((qb, prod.stock_unit.clone(), pu.to_base(Decimal::ONE, &prod.stock_unit)?));
        subtotal += l.line_price;
    }
    if p.discount > subtotal {
        return Err(AppError::invalid("discount", "Discount is larger than the invoice subtotal."));
    }
    let extra = p.shipping + p.nonrecoverable_tax - p.discount;
    let shares = crate::domain::money::allocate(extra, &p.lines.iter().map(|l| l.line_price).collect::<Vec<_>>());
    let lines = p
        .lines
        .iter()
        .zip(shares)
        .zip(qtys)
        .map(|((l, share), (qb, su, size))| {
            let landed = l.line_price + share;
            LinePreview { qty_base: qb, qty_stock: qb / size, stock_unit: su, landed_cost: landed, unit_cost: landed * size / qb }
        })
        .collect();
    Ok(PurchasePreview { subtotal, total: round_money(subtotal + extra), lines })
}

pub fn receive(conn: &Connection, p: &PurchaseInput) -> AppResult<i64> {
    valid_date("purchase_date", &p.purchase_date)?;
    if !matches!(p.kind.as_str(), "purchase" | "opening_balance") {
        return Err(AppError::invalid("kind", "Unknown purchase type."));
    }
    let pv = preview_purchase(conn, p)?;
    conn.execute(
        "INSERT INTO purchases (kind, supplier_id, purchase_date, invoice_ref, subtotal, discount, shipping, nonrecoverable_tax, total, attachment_id, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            p.kind,
            p.supplier_id,
            p.purchase_date,
            p.invoice_ref.trim(),
            pv.subtotal.to_string(),
            p.discount.to_string(),
            p.shipping.to_string(),
            p.nonrecoverable_tax.to_string(),
            pv.total.to_string(),
            p.attachment_id,
            p.notes
        ],
    )?;
    let pid = conn.last_insert_rowid();
    let kind = if p.kind == "opening_balance" { "opening" } else { "purchase" };
    for (l, lp) in p.lines.iter().zip(&pv.lines) {
        conn.execute(
            "INSERT INTO purchase_lines (purchase_id, product_id, package_count, contents_per_package, unit, qty_base, line_price, landed_cost, storage_id, lot_code, expires_on)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                pid,
                l.product_id,
                l.package_count.to_string(),
                l.contents_per_package.to_string(),
                l.unit,
                lp.qty_base.normalize().to_string(),
                l.line_price.to_string(),
                lp.landed_cost.to_string(),
                l.storage_id,
                l.lot_code.trim(),
                l.expires_on
            ],
        )?;
        let line_id = conn.last_insert_rowid();
        post(
            conn,
            l.product_id,
            Op::Receive { kind, qty: lp.qty_base, cost: lp.landed_cost },
            PostCtx { occurred_on: &p.purchase_date, storage_id: l.storage_id, source: Some(("purchase_line", line_id)), note: p.invoice_ref.trim() },
        )?;
    }
    audit(conn, "purchase", Some(pid), "create", &format!("Received {} line(s), total {}", p.lines.len(), pv.total), Some(serde_json::to_value(p)?))?;
    Ok(pid)
}

pub fn reverse_purchase(conn: &Connection, id: i64, date: &str, note: &str) -> AppResult<()> {
    if note.trim().is_empty() {
        return Err(AppError::invalid("note", "Say why this purchase is being reversed."));
    }
    let n = conn.execute("UPDATE purchases SET reversed_at = ?1, reversal_note = ?2 WHERE id = ?3 AND reversed_at IS NULL", params![now_utc(), note.trim(), id])?;
    if n == 0 {
        return Err(AppError::Conflict("This purchase is already reversed (or doesn't exist).".into()));
    }
    let mut st = conn.prepare(
        "SELECT l.id, l.product_id FROM stock_ledger l JOIN purchase_lines pl ON l.source = 'purchase_line' AND l.source_id = pl.id
         WHERE pl.purchase_id = ?1 AND l.kind IN ('purchase','opening')",
    )?;
    let entries: Vec<(i64, i64)> = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    for (entry, product_id) in entries {
        post(conn, product_id, Op::Reverse { entry_id: entry }, PostCtx { occurred_on: date, storage_id: None, source: None, note: note.trim() })?;
    }
    audit(conn, "purchase", Some(id), "reverse", &format!("Purchase {id} reversed: {}", note.trim()), None)?;
    Ok(())
}

#[derive(Serialize, Debug, Clone)]
pub struct PurchaseLineRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub package_count: Decimal,
    pub contents_per_package: Decimal,
    pub unit: String,
    pub qty_stock: Decimal,
    pub stock_unit: String,
    pub line_price: Decimal,
    pub landed_cost: Decimal,
    pub unit_cost: Decimal,
    pub storage_name: Option<String>,
    pub lot_code: String,
    pub expires_on: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct PurchaseRow {
    pub id: i64,
    pub kind: String,
    pub supplier_id: Option<i64>,
    pub supplier_name: Option<String>,
    pub purchase_date: String,
    pub invoice_ref: String,
    pub subtotal: Decimal,
    pub discount: Decimal,
    pub shipping: Decimal,
    pub nonrecoverable_tax: Decimal,
    pub total: Decimal,
    pub attachment_id: Option<i64>,
    pub attachment_name: Option<String>,
    pub notes: String,
    pub reversed_at: Option<String>,
    pub reversal_note: Option<String>,
    pub lines: Vec<PurchaseLineRow>,
}

pub fn purchases(conn: &Connection, from: Option<&str>, to: Option<&str>, product_id: Option<i64>) -> AppResult<Vec<PurchaseRow>> {
    let mut st = conn.prepare(
        "SELECT p.id, p.kind, p.supplier_id, s.name, p.purchase_date, p.invoice_ref, p.subtotal, p.discount, p.shipping, p.nonrecoverable_tax, p.total,
                p.attachment_id, a.file_name, p.notes, p.reversed_at, p.reversal_note
         FROM purchases p LEFT JOIN suppliers s ON s.id = p.supplier_id LEFT JOIN attachments a ON a.id = p.attachment_id
         WHERE (?1 IS NULL OR p.purchase_date >= ?1) AND (?2 IS NULL OR p.purchase_date <= ?2)
           AND (?3 IS NULL OR EXISTS (SELECT 1 FROM purchase_lines x WHERE x.purchase_id = p.id AND x.product_id = ?3))
         ORDER BY p.purchase_date DESC, p.id DESC",
    )?;
    let mut rows = st.query(params![from, to, product_id])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(PurchaseRow {
            id: r.get(0)?,
            kind: r.get(1)?,
            supplier_id: r.get(2)?,
            supplier_name: r.get(3)?,
            purchase_date: r.get(4)?,
            invoice_ref: r.get(5)?,
            subtotal: dec(r, "subtotal")?,
            discount: dec(r, "discount")?,
            shipping: dec(r, "shipping")?,
            nonrecoverable_tax: dec(r, "nonrecoverable_tax")?,
            total: dec(r, "total")?,
            attachment_id: r.get(11)?,
            attachment_name: r.get(12)?,
            notes: r.get(13)?,
            reversed_at: r.get(14)?,
            reversal_note: r.get(15)?,
            lines: vec![],
        });
    }
    let custom = custom_units_all(conn)?;
    let mut lst = conn.prepare(
        "SELECT pl.id, pl.product_id, p.name, pl.package_count, pl.contents_per_package, pl.unit, pl.qty_base, pl.line_price, pl.landed_cost,
                s.name, pl.lot_code, pl.expires_on, p.dimension, p.stock_unit, p.density_g_per_ml
         FROM purchase_lines pl JOIN products p ON p.id = pl.product_id LEFT JOIN storage_locations s ON s.id = pl.storage_id
         WHERE pl.purchase_id = ?1 ORDER BY pl.id",
    )?;
    for p in out.iter_mut() {
        let mut rows = lst.query([p.id])?;
        while let Some(r) = rows.next()? {
            let pid: i64 = r.get(1)?;
            let pu = ProductUnits {
                dimension: Some(Dimension::parse(&r.get::<_, String>(12)?)?),
                density_g_per_ml: opt_dec(r, "density_g_per_ml")?,
                custom: custom.get(&pid).cloned().unwrap_or_default(),
            };
            let su: String = r.get(13)?;
            let size = pu.to_base(Decimal::ONE, &su)?;
            let qb = dec(r, "qty_base")?;
            let landed = dec(r, "landed_cost")?;
            p.lines.push(PurchaseLineRow {
                id: r.get(0)?,
                product_id: pid,
                product_name: r.get(2)?,
                package_count: dec(r, "package_count")?,
                contents_per_package: dec(r, "contents_per_package")?,
                unit: r.get(5)?,
                qty_stock: qb / size,
                line_price: dec(r, "line_price")?,
                landed_cost: landed,
                unit_cost: if qb.is_zero() { Decimal::ZERO } else { landed * size / qb },
                storage_name: r.get(9)?,
                lot_code: r.get(10)?,
                expires_on: r.get(11)?,
                stock_unit: su,
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- alerts

#[derive(Serialize, Debug, Clone)]
pub struct ReorderItem {
    pub product_id: i64,
    pub name: String,
    pub brand: String,
    pub supplier_name: Option<String>,
    pub on_hand: Decimal,
    pub reorder_point: Decimal,
    pub suggested_qty: Decimal,
    pub stock_unit: String,
    pub last_unit_cost: Option<Decimal>,
}

pub fn reorder_list(conn: &Connection) -> AppResult<Vec<ReorderItem>> {
    Ok(products(conn, false)?
        .into_iter()
        .filter(|p| p.low_stock)
        .map(|p| {
            let rp = p.reorder_point.unwrap_or_default();
            let suggested = p.reorder_qty.filter(|q| *q > Decimal::ZERO).unwrap_or_else(|| (rp - p.on_hand).max(Decimal::ZERO) + rp);
            ReorderItem {
                product_id: p.id,
                name: p.name.clone(),
                brand: p.brand.clone(),
                supplier_name: p.supplier_name.clone(),
                on_hand: p.on_hand,
                reorder_point: rp,
                suggested_qty: suggested,
                stock_unit: p.stock_unit.clone(),
                last_unit_cost: p.avg_cost,
            }
        })
        .collect())
}

#[derive(Serialize, Debug, Clone)]
pub struct ExpiringLot {
    pub purchase_line_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub lot_code: String,
    pub expires_on: String,
    pub received_on: String,
    pub qty_received: Decimal,
    pub stock_unit: String,
    pub on_hand: Decimal,
}

/// Lots with an expiry on or before `until` for products that still have stock. Moving-average
/// costing doesn't track which lot was used, so these lots *may* still be on the shelf.
pub fn expiring_lots(conn: &Connection, until: &str) -> AppResult<Vec<ExpiringLot>> {
    valid_date("until", until)?;
    let prods: HashMap<i64, ProductRow> = products(conn, false)?.into_iter().map(|p| (p.id, p)).collect();
    let mut st = conn.prepare(
        "SELECT pl.id, pl.product_id, pl.lot_code, pl.expires_on, pu.purchase_date, pl.qty_base FROM purchase_lines pl JOIN purchases pu ON pu.id = pl.purchase_id
         WHERE pl.expires_on IS NOT NULL AND pl.expires_on <= ?1 AND pu.reversed_at IS NULL ORDER BY pl.expires_on",
    )?;
    let mut rows = st.query([until])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let pid: i64 = r.get(1)?;
        let Some(p) = prods.get(&pid) else { continue };
        if p.on_hand_base <= Decimal::ZERO {
            continue;
        }
        let size = p.units().to_base(Decimal::ONE, &p.stock_unit)?;
        out.push(ExpiringLot {
            purchase_line_id: r.get(0)?,
            product_id: pid,
            product_name: p.name.clone(),
            lot_code: r.get(2)?,
            expires_on: r.get(3)?,
            received_on: r.get(4)?,
            qty_received: dec(r, "qty_base")? / size,
            stock_unit: p.stock_unit.clone(),
            on_hand: p.on_hand,
        });
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::open_memory;
    use rust_decimal_macros::dec;

    pub fn product_input(name: &str, unit: &str) -> ProductInput {
        ProductInput {
            id: None,
            name: name.into(),
            brand: "Acme".into(),
            category: "professional".into(),
            subcategory: String::new(),
            sku: String::new(),
            barcode: String::new(),
            supplier_id: None,
            stock_unit: unit.into(),
            density_g_per_ml: None,
            default_storage_id: None,
            reorder_point: None,
            reorder_qty: None,
            retail_price: None,
            notes: String::new(),
            custom_units: vec![],
        }
    }

    pub fn buy(conn: &Connection, product_id: i64, qty: Decimal, unit: &str, price: Decimal) -> i64 {
        receive(
            conn,
            &PurchaseInput {
                kind: "purchase".into(),
                supplier_id: None,
                purchase_date: "2026-10-01".into(),
                invoice_ref: "INV-1".into(),
                discount: dec!(0),
                shipping: dec!(0),
                nonrecoverable_tax: dec!(0),
                attachment_id: None,
                notes: String::new(),
                lines: vec![PurchaseLineInput {
                    product_id,
                    package_count: dec!(1),
                    contents_per_package: qty,
                    unit: unit.into(),
                    line_price: price,
                    storage_id: None,
                    lot_code: String::new(),
                    expires_on: None,
                }],
            },
        )
        .unwrap()
    }

    #[test]
    fn acceptance_receive_and_consume() {
        let conn = open_memory();
        let id = save_product(&conn, &product_input("Developer 20 vol", "fl_oz")).unwrap();
        buy(&conn, id, dec!(32), "fl_oz", dec!(24));
        let p = product(&conn, id).unwrap();
        assert_eq!(p.on_hand, dec!(32));
        assert_eq!(p.avg_cost, Some(dec!(0.75)));
        let q = p.units().to_base(dec!(2), "fl_oz").unwrap();
        let posted = post(&conn, id, Op::Issue { kind: "service_use", qty: q }, PostCtx { occurred_on: "2026-10-02", storage_id: None, source: Some(("sale_usage", 1)), note: "" }).unwrap();
        assert_eq!(posted.value, dec!(-1.5));
        assert_eq!(product(&conn, id).unwrap().on_hand, dec!(30));
        // the same source can't be deducted twice
        let again = post(&conn, id, Op::Issue { kind: "service_use", qty: q }, PostCtx { occurred_on: "2026-10-02", storage_id: None, source: Some(("sale_usage", 1)), note: "" });
        assert!(matches!(again, Err(AppError::Conflict(_))));
    }

    #[test]
    fn purchase_with_extras_and_reversal() {
        let conn = open_memory();
        let a = save_product(&conn, &product_input("Color A", "g")).unwrap();
        let b = save_product(&conn, &product_input("Foil", "piece")).unwrap();
        let input = PurchaseInput {
            kind: "purchase".into(),
            supplier_id: None,
            purchase_date: "2026-10-01".into(),
            invoice_ref: "X".into(),
            discount: dec!(10),
            shipping: dec!(6),
            nonrecoverable_tax: dec!(4),
            attachment_id: None,
            notes: String::new(),
            lines: vec![
                PurchaseLineInput { product_id: a, package_count: dec!(6), contents_per_package: dec!(2), unit: "oz_wt".into(), line_price: dec!(60), storage_id: None, lot_code: "L1".into(), expires_on: Some("2027-01-01".into()) },
                PurchaseLineInput { product_id: b, package_count: dec!(2), contents_per_package: dec!(500), unit: "piece".into(), line_price: dec!(40), storage_id: None, lot_code: String::new(), expires_on: None },
            ],
        };
        let pv = preview_purchase(&conn, &input).unwrap();
        assert_eq!(pv.total, dec!(100)); // 100 - 10 + 6 + 4
        assert_eq!(pv.lines[0].landed_cost, dec!(60)); // extras net to zero
        let id = receive(&conn, &input).unwrap();
        assert_eq!(product(&conn, a).unwrap().on_hand, dec!(340.19427750)); // 12 oz in grams
        assert_eq!(product(&conn, b).unwrap().on_hand, dec!(1000));
        reverse_purchase(&conn, id, "2026-10-03", "Wrong supplier").unwrap();
        assert_eq!(product(&conn, a).unwrap().on_hand, dec!(0));
        assert_eq!(product(&conn, a).unwrap().value, dec!(0));
        assert!(reverse_purchase(&conn, id, "2026-10-03", "again").is_err());
    }

    #[test]
    fn movements_and_reversals() {
        let conn = open_memory();
        let id = save_product(&conn, &product_input("Shampoo", "ml")).unwrap();
        buy(&conn, id, dec!(1), "l", dec!(20));
        let m = |kind: &str, qty: Decimal| MovementInput {
            product_id: id,
            kind: kind.into(),
            qty,
            unit: "ml".into(),
            storage_id: None,
            to_storage_id: None,
            unit_cost: None,
            occurred_on: "2026-10-02".into(),
            note: "spill".into(),
        };
        let w = record_movement(&conn, &m("waste", dec!(100))).unwrap().unwrap();
        assert_eq!(w.value, dec!(-2));
        let adj = record_movement(&conn, &m("adjustment", dec!(850))).unwrap().unwrap(); // counted 850, had 900
        assert_eq!(adj.qty, dec!(-50));
        reverse_entry(&conn, w.ledger_id, "2026-10-03", "not actually spilled").unwrap();
        assert_eq!(product(&conn, id).unwrap().on_hand, dec!(950));
        assert!(reverse_entry(&conn, w.ledger_id, "2026-10-03", "twice").is_err());
        let rows = ledger(&conn, &LedgerFilter { product_id: Some(id), ..Default::default() }).unwrap();
        // ledger reconciles with the product totals
        let q: Decimal = rows.iter().map(|r| r.qty_base).sum();
        let v: Decimal = rows.iter().map(|r| r.value).sum();
        let p = product(&conn, id).unwrap();
        assert_eq!(q, p.on_hand_base);
        assert_eq!(v, p.value);
    }

    #[test]
    fn transfers_move_quantity_not_value() {
        let conn = open_memory();
        let s1 = save_storage(&conn, &StorageLocation { id: None, name: "Back".into(), location_id: None, archived: false }).unwrap();
        let s2 = save_storage(&conn, &StorageLocation { id: None, name: "Bar".into(), location_id: None, archived: false }).unwrap();
        let id = save_product(&conn, &product_input("Gloves", "piece")).unwrap();
        let mut p = PurchaseInput {
            kind: "purchase".into(),
            supplier_id: None,
            purchase_date: "2026-10-01".into(),
            invoice_ref: String::new(),
            discount: dec!(0),
            shipping: dec!(0),
            nonrecoverable_tax: dec!(0),
            attachment_id: None,
            notes: String::new(),
            lines: vec![],
        };
        p.lines.push(PurchaseLineInput { product_id: id, package_count: dec!(1), contents_per_package: dec!(100), unit: "piece".into(), line_price: dec!(10), storage_id: Some(s1), lot_code: String::new(), expires_on: None });
        receive(&conn, &p).unwrap();
        transfer(&conn, id, dec!(30), Some(s1), Some(s2), "2026-10-02", "").unwrap();
        let by = stock_by_storage(&conn, id).unwrap();
        assert_eq!(by.iter().find(|s| s.storage_id == Some(s1)).unwrap().qty, dec!(70));
        assert_eq!(by.iter().find(|s| s.storage_id == Some(s2)).unwrap().qty, dec!(30));
        assert_eq!(product(&conn, id).unwrap().value, dec!(10));
    }

    #[test]
    fn duplicate_sku_and_dimension_lock() {
        let conn = open_memory();
        let mut a = product_input("A", "ml");
        a.sku = "SKU1".into();
        let id = save_product(&conn, &a).unwrap();
        let mut b = product_input("B", "ml");
        b.sku = "SKU1".into();
        assert!(save_product(&conn, &b).is_err());
        buy(&conn, id, dec!(10), "ml", dec!(1));
        let mut a2 = a.clone();
        a2.id = Some(id);
        a2.stock_unit = "g".into();
        assert!(save_product(&conn, &a2).is_err());
        a2.stock_unit = "fl_oz".into(); // same dimension is fine
        save_product(&conn, &a2).unwrap();
    }
}
