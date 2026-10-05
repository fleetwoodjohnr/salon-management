//! Offline place lookup from the bundled Census Gazetteer 2025 files (public domain): ZIP code →
//! ZCTA internal point, state + city → place internal point. Used when there's no street address
//! or the online geocoder can't be reached. These are area centres, labelled as such.

const ZCTA: &str = include_str!("../data/zcta_2025.txt");
const PLACES: &str = include_str!("../data/places_2025.txt");

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub latitude: String,
    pub longitude: String,
    /// "zcta" or "place"
    pub precision: &'static str,
    pub label: String,
}

pub fn zcta(zip: &str) -> Option<Point> {
    let z = zip.get(..5)?;
    if !z.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    ZCTA.lines().find(|l| l.starts_with(z) && l.as_bytes().get(5) == Some(&b' ')).map(|l| {
        let mut p = l.split(' ').skip(1);
        Point {
            latitude: p.next().unwrap_or_default().into(),
            longitude: p.next().unwrap_or_default().into(),
            precision: "zcta",
            label: format!("Census Gazetteer 2025: centre of ZCTA {z} (a Census area approximating ZIP {z})"),
        }
    })
}

pub fn place(state: &str, city: &str) -> Option<Point> {
    let (st, c) = (state.trim().to_uppercase(), city.trim().to_lowercase());
    if st.len() != 2 || c.is_empty() {
        return None;
    }
    PLACES.lines().filter(|l| !l.starts_with('#')).find_map(|l| {
        let f: Vec<&str> = l.split('|').collect();
        (f.len() == 5 && f[0] == st && (f[1].to_lowercase() == c || f[2].to_lowercase() == c)).then(|| Point {
            latitude: f[3].into(),
            longitude: f[4].into(),
            precision: "place",
            label: format!("Census Gazetteer 2025: centre of {}, {st}", f[2]),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_lookups() {
        let z = zcta("98501").unwrap();
        assert_eq!(z.latitude, "46.9785");
        assert!(zcta("00000").is_none());
        assert!(zcta("9850").is_none());
        let p = place("wa", "Tumwater").unwrap();
        assert!(p.label.contains("Tumwater city"));
        assert!(place("WA", "Nowhereville").is_none());
    }
}
