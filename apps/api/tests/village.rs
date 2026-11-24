use pranata::region_codes::{TimeZone, VillageCode, VillageCodeError, lookup};

fn code(s: &str) -> VillageCode {
    VillageCode::parse(s).expect("valid code")
}

#[test]
fn parse_accepts_dotted_and_plain() {
    assert_eq!(code("14.05.03.2002"), code("1405032002"));
    assert_eq!(code("1405032002").as_str(), "14.05.03.2002");
}

#[test]
fn parse_rejects_nine_digits() {
    assert_eq!(
        VillageCode::parse("140503200"),
        Err(VillageCodeError::Format)
    );
    assert_eq!(
        VillageCode::parse("14.05.03.20x2"),
        Err(VillageCodeError::Format)
    );
}

#[test]
fn lookup_finds_a_riau_village() {
    let record = lookup(&code("14.05.03.2002")).expect("village exists");
    assert_eq!(record.village, "Sorek II");
    assert_eq!(record.district, "Pangkalan Kuras");
    assert_eq!(record.regency, "Kabupaten Pelalawan");
    assert_eq!(record.province, "Riau");
    assert_eq!(record.time_zone, TimeZone::Wib);
}

#[test]
fn lookup_maps_papua_to_wit() {
    let record = lookup(&code("91.03.01.1001")).expect("village exists");
    assert_eq!(record.province, "Papua");
    assert_eq!(record.time_zone, TimeZone::Wit);
    assert_eq!(record.time_zone.iana_name(), "Asia/Jayapura");
}

#[test]
fn lookup_returns_none_for_unknown_code() {
    assert!(lookup(&code("99.99.99.9999")).is_none());
}

#[test]
fn time_zone_follows_province_code() {
    assert_eq!(TimeZone::from_province_code("51"), TimeZone::Wita);
    assert_eq!(TimeZone::from_province_code("76"), TimeZone::Wita);
    assert_eq!(TimeZone::from_province_code("81"), TimeZone::Wit);
    assert_eq!(TimeZone::from_province_code("97"), TimeZone::Wit);
    assert_eq!(TimeZone::from_province_code("14"), TimeZone::Wib);
    assert_eq!(TimeZone::Wita.iana_name(), "Asia/Makassar");
    assert_eq!(TimeZone::Wib.iana_name(), "Asia/Jakarta");
}
