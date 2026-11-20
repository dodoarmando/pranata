//! Slugs: the last part of a website item's URL, made from its title.

/// `text` transliterated to lowercase ASCII words joined by `-`, at most 80 characters;
/// `fallback` when nothing is left.
pub fn of(text: &str, fallback: &str) -> String {
    let mut slug = String::new();
    for c in text.to_lowercase().chars() {
        match c {
            'a'..='z' | '0'..='9' => slug.push(c),
            // "Jum'at" reads as one word.
            '\'' | '’' => {}
            _ => match latin(c) {
                Some(ascii) => slug.push_str(ascii),
                None if !slug.ends_with('-') => slug.push('-'),
                None => {}
            },
        }
    }
    let mut slug = slug.trim_matches('-').to_owned();
    if slug.len() > 80 {
        slug.truncate(80);
        slug.truncate(slug.trim_end_matches('-').len());
    }
    if slug.is_empty() {
        fallback.to_owned()
    } else {
        slug
    }
}

/// The plain letters of an accented Latin letter.
fn latin(c: char) -> Option<&'static str> {
    Some(match c {
        'à'..='å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è'..='ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
        'ğ' => "g",
        'ì'..='ï' | 'ī' | 'į' | 'ı' => "i",
        'ł' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò'..='ö' | 'ø' | 'ō' | 'ő' => "o",
        'œ' => "oe",
        'ř' => "r",
        'ś' | 'š' | 'ş' => "s",
        'ß' => "ss",
        'ť' => "t",
        'ù'..='ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => return None,
    })
}

/// The first of `base`, `base-2`, `base-3`… that is not in `taken`.
pub fn first_free(base: &str, taken: &[String]) -> String {
    std::iter::once(base.to_owned())
        // Among these `taken.len() + 1` candidates, one is always free.
        .chain((2..=taken.len() + 1).map(|n| format!("{base}-{n}")))
        .find(|slug| !taken.contains(slug))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_become_lowercase_ascii_words() {
        assert_eq!(
            of("  Musyawarah Desa: RKPDes 2027!  ", "x"),
            "musyawarah-desa-rkpdes-2027"
        );
        assert_eq!(
            of("Jum'at Bersih — Café Désa", "x"),
            "jumat-bersih-cafe-desa"
        );
        assert_eq!(of("Ñoño & Łódź", "x"), "nono-lodz");
        assert_eq!(of("!!! ???", "berita"), "berita");
        assert_eq!(of("", "berita"), "berita");
    }

    #[test]
    fn long_titles_are_cut_at_80_characters_without_a_trailing_dash() {
        let slug = of(&format!("{} akhir", "a".repeat(79)), "x");
        assert_eq!(slug, "a".repeat(79));
        assert_eq!(of(&"kata ".repeat(40), "x").len(), 79);
    }

    #[test]
    fn a_taken_slug_gets_the_first_free_number_from_2() {
        let taken = [
            "rapat".to_owned(),
            "rapat-2".to_owned(),
            "rapat-4".to_owned(),
        ];
        assert_eq!(first_free("musdes", &taken), "musdes");
        assert_eq!(first_free("rapat", &taken), "rapat-3");
    }
}
