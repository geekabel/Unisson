//! The words the Rust side shows to the user, in each language.
//!
//! Most of the interface is the web page, which has its own texts. A few
//! words are shown from here: the menu of the tray icon, the
//! notifications, the titles of the small windows, and the dialog of a
//! failed start. They are kept together so that a translation has one
//! place to look at.

/// A language the interface is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// French.
    French,
    /// English. It is also the language of anyone the others do not fit.
    English,
}

impl Language {
    /// Every language, in the order the settings screen lists them.
    pub const ALL: [Self; 2] = [Self::French, Self::English];

    /// Returns the two-letter code used in the settings file and by the
    /// interface.
    pub fn id(self) -> &'static str {
        match self {
            Self::French => "fr",
            Self::English => "en",
        }
    }

    /// Finds a language from its [`id`](Self::id).
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|language| language.id() == id)
    }

    /// Picks the language for a locale of the system, such as `fr_FR.UTF-8`
    /// or `en-GB`. Anything that is not known gets English.
    pub fn for_locale(locale: &str) -> Self {
        let code: String = locale
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_ascii_lowercase();
        Self::from_id(&code).unwrap_or(Self::English)
    }

    /// Returns the language of the desktop.
    pub fn of_system() -> Self {
        sys_locale::get_locale().map_or(Self::English, |locale| Self::for_locale(&locale))
    }

    /// Returns the words of this language.
    pub fn texts(self) -> &'static Texts {
        match self {
            Self::French => &FRENCH,
            Self::English => &ENGLISH,
        }
    }
}

/// The language of the interface: the one of the desktop, or a fixed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageChoice {
    /// Like the desktop.
    #[default]
    System,
    /// Always this language.
    Fixed(Language),
}

impl LanguageChoice {
    /// Returns the name used in the settings file and by the interface:
    /// `system`, or the code of a language.
    pub fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Fixed(language) => language.id(),
        }
    }

    /// Finds a choice from its [`id`](Self::id).
    pub fn from_id(id: &str) -> Option<Self> {
        if id == "system" {
            return Some(Self::System);
        }
        Language::from_id(id).map(Self::Fixed)
    }

    /// Returns the language to use.
    pub fn resolve(self) -> Language {
        match self {
            Self::System => Language::of_system(),
            Self::Fixed(language) => language,
        }
    }
}

/// The words of one language.
#[derive(Debug)]
pub struct Texts {
    /// Tray menu: nothing is selected.
    pub nothing_playing: &'static str,
    /// Tray menu: start playback.
    pub play: &'static str,
    /// Tray menu: suspend playback.
    pub pause: &'static str,
    /// Tray menu: next track.
    pub next: &'static str,
    /// Tray menu: previous track.
    pub previous: &'static str,
    /// Tray menu: the title of the list of favorite stations.
    pub favorite_stations: &'static str,
    /// Tray menu: show the main window.
    pub open: &'static str,
    /// Tray menu, and the name of the keyboard shortcut in GNOME.
    pub launcher: &'static str,
    /// Tray menu: show the mini-player.
    pub mini_player: &'static str,
    /// Tray menu: end the application.
    pub quit: &'static str,
    /// The title of the mini-player window.
    pub mini_title: &'static str,
    /// The title of the launcher window.
    pub launcher_title: &'static str,
    /// Notification: playback stopped on a failure.
    pub playback_stopped: &'static str,
    /// Notification: the media no longer exists.
    pub reason_not_found: &'static str,
    /// Notification: the network did not answer.
    pub reason_network: &'static str,
    /// Notification: the source cannot be used right now.
    pub reason_unavailable: &'static str,
    /// Notification: the audio format cannot be decoded.
    pub reason_unsupported: &'static str,
    /// Notification: any other playback failure.
    pub reason_playback: &'static str,
    /// What goes between a title and the reason, with its spaces.
    pub colon: &'static str,
    /// Start failure: the title of the dialog.
    pub cannot_start: &'static str,
    /// Start failure: the audio engine is missing.
    pub cannot_start_audio: &'static str,
    /// Start failure: the database cannot be opened.
    pub cannot_start_store: &'static str,
    /// Start failure: anything else.
    pub cannot_start_other: &'static str,
    /// Start failure: what comes before the technical reason.
    pub technical_detail: &'static str,
}

static FRENCH: Texts = Texts {
    nothing_playing: "Rien en lecture",
    play: "Lecture",
    pause: "Pause",
    next: "Suivant",
    previous: "Précédent",
    favorite_stations: "Radios favorites",
    open: "Ouvrir Unisson",
    launcher: "Lanceur rapide",
    mini_player: "Mini-lecteur",
    quit: "Quitter",
    mini_title: "Unisson, mini-lecteur",
    launcher_title: "Unisson, lanceur rapide",
    playback_stopped: "Lecture interrompue",
    reason_not_found: "introuvable",
    reason_network: "le réseau ne répond pas",
    reason_unavailable: "source indisponible",
    reason_unsupported: "format non pris en charge",
    reason_playback: "lecture impossible",
    colon: " : ",
    cannot_start: "Unisson ne peut pas démarrer",
    cannot_start_audio: "Le moteur audio GStreamer n'a pas pu démarrer.\n\n\
        Vérifiez que GStreamer et ses greffons sont installés.",
    cannot_start_store: "La base de données d'Unisson n'a pas pu être ouverte.\n\n\
        Vérifiez que vous pouvez écrire dans votre dossier personnel, et que \
        le disque n'est pas plein.",
    cannot_start_other: "Une erreur inattendue empêche le démarrage.",
    technical_detail: "Détail technique : ",
};

static ENGLISH: Texts = Texts {
    nothing_playing: "Nothing playing",
    play: "Play",
    pause: "Pause",
    next: "Next",
    previous: "Previous",
    favorite_stations: "Favorite stations",
    open: "Open Unisson",
    launcher: "Quick launcher",
    mini_player: "Mini-player",
    quit: "Quit",
    mini_title: "Unisson, mini-player",
    launcher_title: "Unisson, quick launcher",
    playback_stopped: "Playback stopped",
    reason_not_found: "not found",
    reason_network: "the network does not answer",
    reason_unavailable: "source unavailable",
    reason_unsupported: "unsupported format",
    reason_playback: "cannot be played",
    colon: ": ",
    cannot_start: "Unisson cannot start",
    cannot_start_audio: "The GStreamer audio engine could not start.\n\n\
        Check that GStreamer and its plugins are installed.",
    cannot_start_store: "The database of Unisson could not be opened.\n\n\
        Check that you can write in your home folder, and that the disk is \
        not full.",
    cannot_start_other: "An unexpected error prevents the start.",
    technical_detail: "Technical detail: ",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locale_of_the_system_picks_a_language() {
        for locale in ["fr_FR.UTF-8", "fr-CA", "fr", "FR_be"] {
            assert_eq!(Language::for_locale(locale), Language::French, "{locale}");
        }
        for locale in ["en_US.UTF-8", "en-GB", "de_DE.UTF-8", "C", "POSIX", ""] {
            assert_eq!(Language::for_locale(locale), Language::English, "{locale}");
        }
    }

    #[test]
    fn a_choice_is_found_from_its_id() {
        assert_eq!(
            LanguageChoice::from_id("system"),
            Some(LanguageChoice::System)
        );
        for language in Language::ALL {
            let choice = LanguageChoice::Fixed(language);
            assert_eq!(LanguageChoice::from_id(choice.id()), Some(choice));
            assert_eq!(choice.resolve(), language);
        }
        assert_eq!(LanguageChoice::from_id("klingon"), None);
    }

    #[test]
    fn the_two_languages_differ_where_they_should() {
        let (french, english) = (Language::French.texts(), Language::English.texts());
        assert_ne!(french.nothing_playing, english.nothing_playing);
        assert_ne!(french.cannot_start, english.cannot_start);
        // French puts a space before the colon, English does not.
        assert_eq!(french.colon, " : ");
        assert_eq!(english.colon, ": ");
    }
}
