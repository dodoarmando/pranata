//! Bundled Kepmendagri region codes (D-02). The dataset is embedded in the
//! binary and parsed once, on first lookup; see `data/SOURCE.md`.

use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use serde::Deserialize;
use time::UtcOffset;

static DATA: &[u8] = include_bytes!("../data/region-codes.csv.gz");

/// A village code in the dotted form `PP.KK.CC.VVVV`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VillageCode(String);

/// Why a village code was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VillageCodeError {
    #[error("a village code has exactly 10 digits")]
    Format,
}

impl VillageCode {
    /// Accept `14.05.03.2002` or `1405032002` and normalise to the dotted form.
    ///
    /// # Errors
    /// `VillageCodeError::Format` unless the input is 10 digits, plain or in
    /// the `2.2.2.4` dotted grouping.
    pub fn parse(input: &str) -> Result<Self, VillageCodeError> {
        let digits = if input.contains('.') {
            let groups: Vec<&str> = input.split('.').collect();
            if groups.iter().map(|g| g.len()).collect::<Vec<_>>() != [2, 2, 2, 4] {
                return Err(VillageCodeError::Format);
            }
            groups.concat()
        } else {
            input.to_owned()
        };
        if digits.len() != 10 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(VillageCodeError::Format);
        }
        Ok(Self(format!(
            "{}.{}.{}.{}",
            &digits[0..2],
            &digits[2..4],
            &digits[4..6],
            &digits[6..10]
        )))
    }

    /// The two digit province code.
    #[must_use]
    pub fn province_code(&self) -> &str {
        self.0.get(..2).unwrap_or_default()
    }

    /// The dotted form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VillageCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Indonesian standard time zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZone {
    Wib,
    Wita,
    Wit,
}

impl TimeZone {
    /// The zone a province (two digit code) lies in.
    #[must_use]
    pub fn from_province_code(province: &str) -> Self {
        match province {
            "51" | "52" | "53" | "63" | "64" | "65" | "71" | "72" | "73" | "74" | "75" | "76" => {
                Self::Wita
            }
            "81" | "82" | "91" | "92" | "93" | "94" | "95" | "96" | "97" => Self::Wit,
            _ => Self::Wib,
        }
    }

    /// The IANA time zone name.
    #[must_use]
    pub const fn iana_name(self) -> &'static str {
        match self {
            Self::Wib => "Asia/Jakarta",
            Self::Wita => "Asia/Makassar",
            Self::Wit => "Asia/Jayapura",
        }
    }

    /// The fixed offset from UTC. Indonesia has no daylight saving.
    #[must_use]
    pub const fn utc_offset(self) -> UtcOffset {
        let hours = match self {
            Self::Wib => 7,
            Self::Wita => 8,
            Self::Wit => 9,
        };
        match UtcOffset::from_hms(hours, 0, 0) {
            Ok(offset) => offset,
            Err(_) => UtcOffset::UTC,
        }
    }

    /// Parse an IANA name written by [`TimeZone::iana_name`].
    #[must_use]
    pub fn from_iana_name(name: &str) -> Option<Self> {
        [Self::Wib, Self::Wita, Self::Wit]
            .into_iter()
            .find(|zone| zone.iana_name() == name)
    }
}

/// What the dataset knows about a village.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRecord {
    pub village: String,
    pub district: String,
    pub regency: String,
    pub province: String,
    pub time_zone: TimeZone,
}

#[derive(Deserialize)]
struct Row {
    code: String,
    village: String,
    district: String,
    regency: String,
    province: String,
}

// The bundled data is checked by the `bundled_dataset_is_well_formed` test
// (row count, code shape, no duplicates), so the `expect`s below cannot fire
// for a dataset that passed CI. Duplicate codes are detected here, not
// overwritten.
static TABLE: LazyLock<HashMap<VillageCode, RegionRecord>> = LazyLock::new(|| {
    let mut text = String::new();
    GzDecoder::new(DATA)
        .read_to_string(&mut text)
        .expect("bundled region codes are valid gzip");
    let mut table = HashMap::new();
    for row in csv::Reader::from_reader(text.as_bytes()).deserialize::<Row>() {
        let row = row.expect("bundled region codes are valid csv");
        let code = VillageCode::parse(&row.code).expect("bundled region code is well formed");
        let record = RegionRecord {
            village: row.village,
            district: row.district,
            regency: row.regency,
            province: row.province,
            time_zone: TimeZone::from_province_code(code.province_code()),
        };
        assert!(
            table.insert(code, record).is_none(),
            "duplicate bundled region code {}",
            row.code
        );
    }
    table
});

/// Number of villages in the bundled dataset.
#[must_use]
pub fn record_count() -> usize {
    TABLE.len()
}

/// Every village code in the bundled dataset, in no particular order.
pub fn codes() -> impl Iterator<Item = &'static VillageCode> {
    TABLE.keys()
}

/// Look a village up in the bundled dataset.
#[must_use]
pub fn lookup(code: &VillageCode) -> Option<RegionRecord> {
    TABLE.get(code).cloned()
}
