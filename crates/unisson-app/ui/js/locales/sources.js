/**
 * Texts of the "sources" area. Keys start with `sources.`.
 */

export default {
  fr: {
    "sources.title": "Sources et réglages",
    "sources.settingNotSaved": "Ce réglage n'a pas pu être enregistré.",
    "sources.settingsNotSavable": "Les réglages ne peuvent pas être enregistrés.",

    // Source cards.
    "sources.sectionSources": "Sources",
    "sources.statusOff": "Coupée. Ses résultats n'apparaissent pas dans la recherche.",
    "sources.statusMissingTool": "Active, mais son composant n'est pas installé.",
    "sources.statusOn": "Active. Aucun compte requis.",
    "sources.enableSource": "Activer la source {name}",

    // YouTube.
    "sources.checkAgain": "Vérifier à nouveau",
    "sources.componentInstalled": "Composant YouTube installé, version {version}.",
    "sources.componentMissing": "YouTube a besoin d'un composant qui n'est pas installé sur cet ordinateur.",
    "sources.installGuide": "Le guide d'installation explique comment l'ajouter : ",

    // Spotify.
    "sources.spotifyStatusOff": "Active, mais aucun compte n'est connecté.",
    "sources.spotifyStatusOn": "Active. Compte connecté.",
    "sources.spotifyIntro": "Unisson pilote l'application Spotify officielle. Il faut un compte Premium, et l'application ouverte sur cet ordinateur.",
    "sources.spotifyStep1": "Sur le site des développeurs Spotify, créez une application, avec l'API Web : ",
    "sources.spotifyStep2": "Ajoutez cette adresse de redirection : ",
    "sources.spotifyStep3": "Dans « User Management », ajoutez l'e-mail du compte qui écoute.",
    "sources.spotifyStep4": "Copiez son Client ID ci-dessous, puis connectez-vous.",
    "sources.spotifyClientId": "Client ID de votre application Spotify",
    "sources.spotifyConnect": "Se connecter",
    "sources.spotifyWaiting": "Validez dans votre navigateur…",
    "sources.spotifyConnected": "Gardez l'application Spotify ouverte pour écouter ses titres.",
    "sources.spotifyDisconnect": "Se déconnecter",
    "sources.spotifyInvalidId": "Ce Client ID n'est pas valide. Il compte 32 lettres et chiffres.",
    "sources.spotifyNetwork": "Spotify ne répond pas. Vérifiez votre connexion.",
    "sources.spotifyFailed": "La connexion à Spotify n'a pas abouti. Vérifiez l'adresse de redirection et l'accès du compte, puis réessayez.",
    "sources.spotifyDisconnectFailed": "La déconnexion de Spotify n'a pas pu être enregistrée.",

    // Local files.
    "sources.folder": "Dossier : ",
    "sources.folderDefault": "{path} (dossier Musique du bureau)",
    "sources.noFolder": "aucun dossier connu",
    "sources.folderMissing": "Ce dossier est introuvable.",
    "sources.tracksInLibrary.one": "{count} titre dans la bibliothèque.",
    "sources.tracksInLibrary.other": "{count} titres dans la bibliothèque.",
    "sources.changeFolder": "Changer de dossier",
    "sources.resetFolder": "Revenir au dossier Musique",
    "sources.rescan": "Analyser à nouveau",
    "sources.rescanStarted": "Analyse du dossier en cours.",

    // Priority.
    "sources.sectionPriority": "Priorité des sources",
    "sources.priorityHint": "Si un titre existe dans plusieurs sources, la première de la liste est utilisée.",
    "sources.sourceOff": " (coupée)",
    "sources.moveUp": "Monter {name}",
    "sources.moveDown": "Descendre {name}",

    // Behavior.
    "sources.sectionBehavior": "Comportement",
    "sources.resumeRadio": "Quand la file est vide, reprendre la dernière radio",
    "sources.keepInTray": "À la fermeture de la fenêtre, continuer la lecture",
    "sources.keepInTrayDetail": "L'icône de la barre du haut rouvre la fenêtre, et son menu permet de quitter.",
    "sources.notifications": "Annoncer chaque nouveau titre par une notification",
    "sources.notificationsDetail": "Elle s'affiche seulement quand Unisson n'est pas la fenêtre active.",

    // Appearance.
    "sources.sectionAppearance": "Apparence",
    "sources.sameAsSystem": "Comme le système",
    "sources.theme": "Thème",
    "sources.themeDark": "Sombre",
    "sources.themeLight": "Clair",
    "sources.themeDetail": "« Comme le système » suit le style clair ou sombre choisi dans les réglages du bureau.",
    "sources.language": "Langue",
    "sources.languageDetail": "« Comme le système » suit la langue du bureau.",

    // Quick launcher.
    "sources.sectionLauncher": "Lanceur rapide",
    "sources.launcherIntro": "Le lanceur est une petite fenêtre de recherche qui s'ouvre par-dessus votre travail : quelques lettres, Entrée, et la lecture démarre.",
    "sources.openLauncher": "Ouvrir le lanceur",
    "sources.shortcutManual": "Pour l'ouvrir au clavier, créez dans les réglages de votre bureau un raccourci qui lance cette commande :",
    "sources.shortcutChoice": "Raccourci clavier",
    "sources.changeShortcut": "Changer le raccourci",
    "sources.installShortcut": "Installer le raccourci",
    "sources.removeShortcut": "Retirer",
    "sources.shortcutInPlace": "Raccourci en place : {name}. Il apparaît aussi dans les réglages de GNOME, sous Clavier.",
    "sources.noShortcut": "Aucun raccourci pour l'instant. Unisson peut l'ajouter aux raccourcis personnalisés de GNOME.",

    // Keyboard shortcuts of the main window.
    "sources.sectionShortcuts": "Raccourcis clavier",
  },
  en: {
    "sources.title": "Sources and settings",
    "sources.settingNotSaved": "This setting could not be saved.",
    "sources.settingsNotSavable": "Settings cannot be saved.",

    // Source cards.
    "sources.sectionSources": "Sources",
    "sources.statusOff": "Off. Its results do not appear in search.",
    "sources.statusMissingTool": "On, but its component is not installed.",
    "sources.statusOn": "On. No account needed.",
    "sources.enableSource": "Enable the {name} source",

    // YouTube.
    "sources.checkAgain": "Check again",
    "sources.componentInstalled": "YouTube component installed, version {version}.",
    "sources.componentMissing": "YouTube needs a component that is not installed on this computer.",
    "sources.installGuide": "The installation guide explains how to add it: ",

    // Spotify.
    "sources.spotifyStatusOff": "On, but no account is connected.",
    "sources.spotifyStatusOn": "On. Account connected.",
    "sources.spotifyIntro": "Unisson drives the official Spotify application. It needs a Premium account, and the application open on this computer.",
    "sources.spotifyStep1": "On the Spotify developer site, create an app, with the Web API: ",
    "sources.spotifyStep2": "Add this redirect URI: ",
    "sources.spotifyStep3": "In \u201cUser Management\u201d, add the email of the account that listens.",
    "sources.spotifyStep4": "Copy its Client ID below, then connect.",
    "sources.spotifyClientId": "Client ID of your Spotify app",
    "sources.spotifyConnect": "Connect",
    "sources.spotifyWaiting": "Confirm in your browser…",
    "sources.spotifyConnected": "Keep the Spotify application open to listen to its tracks.",
    "sources.spotifyDisconnect": "Disconnect",
    "sources.spotifyInvalidId": "This Client ID is not valid. It has 32 letters and digits.",
    "sources.spotifyNetwork": "Spotify is not responding. Check your connection.",
    "sources.spotifyFailed": "The connection to Spotify did not work. Check the redirect URI and the access of the account, then try again.",
    "sources.spotifyDisconnectFailed": "The disconnection from Spotify could not be saved.",

    // Local files.
    "sources.folder": "Folder: ",
    "sources.folderDefault": "{path} (the desktop Music folder)",
    "sources.noFolder": "no known folder",
    "sources.folderMissing": "This folder cannot be found.",
    "sources.tracksInLibrary.one": "{count} track in the library.",
    "sources.tracksInLibrary.other": "{count} tracks in the library.",
    "sources.changeFolder": "Change folder",
    "sources.resetFolder": "Back to the Music folder",
    "sources.rescan": "Scan again",
    "sources.rescanStarted": "Scanning the folder.",

    // Priority.
    "sources.sectionPriority": "Source priority",
    "sources.priorityHint": "If a track exists in several sources, the first one in the list is used.",
    "sources.sourceOff": " (off)",
    "sources.moveUp": "Move {name} up",
    "sources.moveDown": "Move {name} down",

    // Behavior.
    "sources.sectionBehavior": "Behavior",
    "sources.resumeRadio": "When the queue is empty, resume the last station",
    "sources.keepInTray": "When the window is closed, keep playing",
    "sources.keepInTrayDetail": "The icon in the top bar reopens the window, and its menu lets you quit.",
    "sources.notifications": "Announce each new track with a notification",
    "sources.notificationsDetail": "It only shows when Unisson is not the active window.",

    // Appearance.
    "sources.sectionAppearance": "Appearance",
    "sources.sameAsSystem": "Same as system",
    "sources.theme": "Theme",
    "sources.themeDark": "Dark",
    "sources.themeLight": "Light",
    "sources.themeDetail": "“Same as system” follows the light or dark style chosen in the desktop settings.",
    "sources.language": "Language",
    "sources.languageDetail": "“Same as system” follows the language of the desktop.",

    // Quick launcher.
    "sources.sectionLauncher": "Quick launcher",
    "sources.launcherIntro": "The launcher is a small search window that opens on top of your work: a few letters, Enter, and playback starts.",
    "sources.openLauncher": "Open the launcher",
    "sources.shortcutManual": "To open it from the keyboard, create a shortcut in your desktop settings that runs this command:",
    "sources.shortcutChoice": "Keyboard shortcut",
    "sources.changeShortcut": "Change the shortcut",
    "sources.installShortcut": "Install the shortcut",
    "sources.removeShortcut": "Remove",
    "sources.shortcutInPlace": "Current shortcut: {name}. It also appears in the GNOME settings, under Keyboard.",
    "sources.noShortcut": "No shortcut yet. Unisson can add it to the GNOME custom shortcuts.",

    // Keyboard shortcuts of the main window.
    "sources.sectionShortcuts": "Keyboard shortcuts",
  },
};
