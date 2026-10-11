//! What happens when Unisson cannot start.
//!
//! A program started from the menu of the desktop has no terminal: a
//! failure written there is never read, and the user only sees an
//! application that does not open. So the reason is shown in a dialog,
//! with what can be done about it, and then the program ends.

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use unisson_audio::AudioError;
use unisson_store::StoreError;

use crate::windows;

/// Explains to the user why Unisson cannot start, then ends the program.
///
/// It returns at once: the dialog needs the main loop, which only runs
/// after the setup of the application returned.
pub fn report_failure(app: &AppHandle, error: &(dyn std::error::Error + 'static)) {
    eprintln!("unisson: cannot start: {error}");
    // The main window would only show an interface that cannot work.
    if let Some(window) = app.get_webview_window(windows::MAIN) {
        let _ = window.hide();
    }
    let text = message(error);
    let dialog = app.clone();
    let spawned = std::thread::Builder::new()
        .name("unisson-startup-failure".into())
        .spawn(move || {
            dialog
                .dialog()
                .message(text)
                .title("Unisson ne peut pas démarrer")
                .kind(MessageDialogKind::Error)
                .blocking_show();
            dialog.exit(1);
        });
    if spawned.is_err() {
        app.exit(1);
    }
}

/// Builds the text of the dialog: what is wrong in plain words, what to
/// do, then the technical reason for a bug report. The text is in French,
/// like the rest of the interface.
fn message(error: &(dyn std::error::Error + 'static)) -> String {
    let advice = if error.downcast_ref::<AudioError>().is_some() {
        "Le moteur audio GStreamer n'a pas pu démarrer.\n\n\
         Vérifiez que ses greffons sont installés :\n\
         sudo apt install gstreamer1.0-plugins-base gstreamer1.0-plugins-good"
    } else if error.downcast_ref::<StoreError>().is_some() {
        "La base de données d'Unisson n'a pas pu être ouverte.\n\n\
         Vérifiez que le dossier ~/.local/share/unisson existe, que vous \
         pouvez y écrire, et que le disque n'est pas plein."
    } else {
        "Une erreur inattendue empêche le démarrage."
    };
    format!("{advice}\n\nDétail technique : {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_gives_advice_and_keeps_the_technical_reason() {
        let audio = AudioError::MissingElement("playbin");
        let text = message(&audio);
        assert!(text.contains("GStreamer"), "{text}");
        assert!(text.contains("sudo apt install"), "{text}");
        assert!(
            text.ends_with(&format!("Détail technique : {audio}")),
            "{text}"
        );

        let other = std::io::Error::other("disk on fire");
        let text = message(&other);
        assert!(text.contains("inattendue"), "{text}");
        assert!(text.contains("disk on fire"), "{text}");
    }
}
