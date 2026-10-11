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

use crate::texts::{Language, Texts};
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
    // The settings were not read: the language of the desktop is used.
    let texts = Language::of_system().texts();
    let text = message(error, texts);
    let dialog = app.clone();
    let spawned = std::thread::Builder::new()
        .name("unisson-startup-failure".into())
        .spawn(move || {
            dialog
                .dialog()
                .message(text)
                .title(texts.cannot_start)
                .kind(MessageDialogKind::Error)
                .blocking_show();
            dialog.exit(1);
        });
    if spawned.is_err() {
        app.exit(1);
    }
}

/// Builds the text of the dialog: what is wrong in plain words, then the
/// technical reason for a bug report.
fn message(error: &(dyn std::error::Error + 'static), texts: &Texts) -> String {
    let advice = if error.downcast_ref::<AudioError>().is_some() {
        texts.cannot_start_audio
    } else if error.downcast_ref::<StoreError>().is_some() {
        texts.cannot_start_store
    } else {
        texts.cannot_start_other
    };
    format!("{advice}\n\n{}{error}", texts.technical_detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_gives_advice_and_keeps_the_technical_reason() {
        let audio = AudioError::MissingElement("playbin");
        for language in Language::ALL {
            let texts = language.texts();
            let text = message(&audio, texts);
            assert!(text.contains("GStreamer"), "{text}");
            assert!(
                text.ends_with(&format!("{}{audio}", texts.technical_detail)),
                "{text}"
            );

            let other = std::io::Error::other("disk on fire");
            let text = message(&other, texts);
            assert!(text.starts_with(texts.cannot_start_other), "{text}");
            assert!(text.contains("disk on fire"), "{text}");
        }
    }
}
