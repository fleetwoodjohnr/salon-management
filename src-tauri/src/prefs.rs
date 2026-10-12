//! App-wide preferences (shared by all workspaces): distance unit, update checks and the last
//! market-research settings. Stored as `preferences.json` in the data root, which installers and
//! updates never touch. Unknown fields are ignored and missing ones take defaults, so older and
//! newer versions can share the file.

use crate::domain::market::DistanceUnit;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

const FILE: &str = "preferences.json";
/// Business types the research tool can look for (see `providers::overpass`).
pub const BUSINESS_TYPES: &[&str] = &["hair", "barber", "nails", "beauty", "massage"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Prefs {
    pub distance_unit: DistanceUnit,
    pub auto_update_check: bool,
    /// UTC timestamp of the last automatic or manual update check.
    pub last_update_check: Option<String>,
    /// Always kilometres, whatever unit is displayed.
    pub research_radius_km: f64,
    pub research_types: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            distance_unit: DistanceUnit::Mi,
            auto_update_check: true,
            last_update_check: None,
            research_radius_km: 5.0 * crate::domain::market::KM_PER_MI,
            research_types: ["hair", "barber", "nails", "beauty"].map(String::from).to_vec(),
        }
    }
}

/// A missing or unreadable file means defaults; preferences never block startup.
pub fn load(root: &Path) -> Prefs {
    std::fs::read_to_string(root.join(FILE)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(root: &Path, p: &Prefs) -> AppResult<()> {
    if !(0.5..=50.0).contains(&p.research_radius_km) {
        return Err(AppError::invalid("research_radius_km", "Search radius must be between 0.5 and 50 km (0.3 to 31 mi)."));
    }
    if p.research_types.is_empty() || p.research_types.iter().any(|t| !BUSINESS_TYPES.contains(&t.as_str())) {
        return Err(AppError::invalid("research_types", "Choose at least one kind of business."));
    }
    let path = root.join(FILE);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(p)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_and_tolerance() {
        let dir = std::env::temp_dir().join(format!("srm-prefs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(load(&dir), Prefs::default());
        // Older/newer files: missing fields default, unknown fields are ignored.
        std::fs::write(dir.join(FILE), r#"{"distance_unit":"km","from_the_future":1}"#).unwrap();
        let p = load(&dir);
        assert_eq!(p.distance_unit, DistanceUnit::Km);
        assert!(p.auto_update_check);
        let mut q = p.clone();
        q.research_radius_km = 12.5;
        save(&dir, &q).unwrap();
        assert_eq!(load(&dir), q);
        q.research_radius_km = 80.0;
        assert!(save(&dir, &q).is_err());
        q.research_radius_km = 10.0;
        q.research_types = vec!["tattoo".into()];
        assert!(save(&dir, &q).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
