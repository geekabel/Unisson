//! Text helpers shared by the sources.

/// Simplifies a text so that two spellings of the same words compare equal.
///
/// The result is in lower case, without accents and without extra spaces.
/// It is meant for searching and comparing, never for display.
///
/// Accents are removed for the Latin letters used in western European
/// languages. Other scripts are only put in lower case.
///
/// # Examples
///
/// ```
/// use unisson_core::search_key;
///
/// assert_eq!(search_key("  Céline   DION "), "celine dion");
/// assert_eq!(search_key("Sigur Rós"), search_key("sigur ros"));
/// ```
pub fn search_key(text: &str) -> String {
    let mut key = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !key.is_empty() {
            key.push(' ');
        }
        for letter in word.chars().flat_map(char::to_lowercase) {
            match unaccented(letter) {
                Some(plain) => key.push_str(plain),
                None => key.push(letter),
            }
        }
    }
    key
}

/// Returns the plain spelling of an accented lower-case letter.
fn unaccented(letter: char) -> Option<&'static str> {
    Some(match letter {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => "a",
        'ç' | 'ć' | 'č' => "c",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'ī' => "i",
        'ñ' | 'ń' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => "o",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' => "u",
        'ý' | 'ÿ' => "y",
        'š' | 'ś' => "s",
        'ž' | 'ź' | 'ż' => "z",
        'ł' => "l",
        'æ' => "ae",
        'œ' => "oe",
        'ß' => "ss",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_accents_and_spaces_are_ignored() {
        assert_eq!(search_key("Beyoncé"), "beyonce");
        assert_eq!(search_key("ÉCOUTE  ça\t!"), "ecoute ca !");
        assert_eq!(search_key("Cœur de Bœuf"), "coeur de boeuf");
        assert_eq!(search_key("Straße"), "strasse");
        assert_eq!(search_key("   "), "");
    }

    #[test]
    fn other_scripts_are_only_lower_cased() {
        assert_eq!(search_key("МУЗЫКА"), "музыка");
        assert_eq!(search_key("音楽 Mix"), "音楽 mix");
    }

    #[test]
    fn the_key_of_a_key_is_itself() {
        let key = search_key("Les Désaxés – Été 2010");
        assert_eq!(search_key(&key), key);
    }
}
