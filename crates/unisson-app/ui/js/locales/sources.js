/**
 * Texts of the "sources" area. Keys start with `sources.`.
 */

export default {
  fr: {
    "sources.title": "Sources",
    "sources.intro": "Chaque source active répond à la recherche. Leur ordre décide laquelle joue un titre trouvé dans plusieurs.",
    "sources.settingNotSaved": "Ce réglage n'a pas pu être enregistré.",

    // Source cards.
    "sources.statusOff": "Coupée. Ses résultats n'apparaissent pas dans la recherche.",
    "sources.statusMissingTool": "Active, mais son composant n'est pas installé.",
    "sources.statusOn": "Active. Aucun compte requis.",
    "sources.enableSource": "Activer la source {name}",
    "sources.copy": "Copier",
    "sources.copied": "Copié dans le presse-papiers.",
    "sources.cancel": "Annuler",

    // YouTube.
    "sources.checkAgain": "Vérifier à nouveau",
    "sources.componentInstalled": "Composant YouTube installé, version {version}.",
    "sources.componentMissing": "YouTube a besoin d'un composant qui n'est pas installé sur cet ordinateur.",
    "sources.installGuide": "Le guide d'installation explique comment l'ajouter : ",

    // Spotify: the head of the card.
    "sources.spotifySignedOut": "Active, mais aucun compte n'est connecté.",
    "sources.spotifyInUnisson": "Active. La musique se joue dans Unisson.",
    "sources.spotifyInApp": "Active. La musique se joue dans l'application Spotify.",
    "sources.spotifyViaApp": "Active. La musique passe pour l'instant par l'application Spotify.",
    "sources.spotifyStopped": "Active, mais la lecture est interrompue.",

    // Spotify: the account.
    "sources.spotifyAccount": "Compte",
    "sources.spotifyNoAccount": "Aucun compte connecté",
    "sources.spotifyPremiumNeeded": "Il faut un compte Spotify Premium.",
    "sources.spotifySignIn": "Se connecter avec Spotify…",
    "sources.spotifyWaiting": "Validez la connexion dans votre navigateur…",
    "sources.spotifyWaitingHint": "La page de Spotify vient de s'ouvrir. Elle n'est pas apparue ? Copiez le lien.",
    "sources.spotifyCopyLink": "Copier le lien",
    "sources.spotifySignedIn": "Compte connecté",
    "sources.spotifySignedInHint": "Spotify Premium est nécessaire pour écouter.",
    "sources.spotifySignOut": "Se déconnecter",
    "sources.spotifyAbout": "La musique se joue dans Unisson, avec son volume et sa file d'attente. Aucune autre application à ouvrir.",

    // Spotify: where it plays.
    "sources.spotifyPlayback": "Lecture",
    "sources.chipInUnisson": "Dans Unisson",
    "sources.chipViaApp": "Par l'application Spotify",
    "sources.chipStopped": "Interrompue",
    "sources.chipRefused": "Pas avec ce compte",
    "sources.chipPremium": "Premium requis",
    "sources.spotifyOkHint": "Aucune autre application à ouvrir.",
    "sources.spotifyAppOnlyHint": "Gardez l'application Spotify ouverte. Connectez-vous avec Spotify pour écouter dans Unisson.",
    "sources.spotifyLimitedViaApp": "Spotify limite la lecture dans Unisson pour le moment. Unisson réessaie tout seul {when}. Gardez l'application Spotify ouverte.",
    "sources.spotifyRefusedViaApp": "Spotify ne laisse pas ce compte lire sa musique dans Unisson. Elle passe par l'application Spotify.",
    "sources.spotifyLimitedStopped": "Spotify refuse la lecture dans Unisson pour le moment. Nouvel essai {when}. Une lecture de secours éviterait cette coupure.",
    "sources.spotifyRefusedStopped": "Spotify ne laisse pas ce compte lire sa musique dans Unisson. C'est fréquent avec les comptes créés depuis fin 2024. La lecture de secours règle le problème.",
    "sources.spotifyPremiumStopped": "Spotify ne laisse écouter qu'avec un compte Premium.",
    "sources.retryIn.one": "dans {count} minute",
    "sources.retryIn.other": "dans {count} minutes",
    "sources.retrySoon": "au prochain titre",
    "sources.spotifyRetryNow": "Réessayer maintenant",
    "sources.spotifyRetry": "Réessayer",
    "sources.spotifySetUpBackup": "Configurer le secours…",

    // Spotify: the backup.
    "sources.backupTitle": "Lecture de secours",
    "sources.backupNone": "Facultatif. Non configurée.",
    "sources.backupReady": "Prête. Utilisée seulement si besoin.",
    "sources.backupInUse": "Prête. Utilisée en ce moment.",
    "sources.backupIntro": "Utile si la lecture dans Unisson échoue. Il faut un compte Premium et environ 5 minutes. La musique passera alors par l'application Spotify.",
    "sources.backupReadyIntro": "Si la lecture dans Unisson échoue, la musique passe par l'application Spotify. Elle doit être ouverte sur cet ordinateur.",
    "sources.backupApp": "Votre application Spotify",
    "sources.backupTest": "Tester",
    "sources.backupRemove": "Retirer",
    "sources.backupStep1": "Créer une application Spotify",
    "sources.backupStep1Detail": "Sur le site des développeurs, donnez-lui un nom et cochez « Web API ».",
    "sources.backupOpenDashboard": "Ouvrir le site des développeurs",
    "sources.backupStep2": "Ajouter l'adresse de redirection",
    "sources.backupStep3": "Autoriser votre compte",
    "sources.backupStep3Detail": "Dans « User Management », ajoutez l'e-mail de ce compte Spotify. Sans cela, Spotify refuse la connexion.",
    "sources.backupStep4": "Coller le Client ID de l'application",
    "sources.backupClientIdPlaceholder": "32 lettres et chiffres",
    "sources.backupSave": "Tester et enregistrer",
    "sources.backupKeep": "Vous pouvez quitter cet écran : ce que vous avez tapé reste là.",
    "sources.autoSwitch": "Basculer automatiquement",
    "sources.autoSwitchDetail": "Seulement sur les pannes connues de la lecture dans Unisson. Unisson vous prévient à chaque fois.",
    "sources.autoSwitchUnavailable": "Disponible une fois la lecture de secours prête.",

    // Spotify: what went wrong.
    "sources.spotifyInvalidId": "Ce Client ID n'est pas valide. Il compte 32 lettres et chiffres.",
    "sources.spotifyNetwork": "Spotify ne répond pas. Vérifiez votre connexion.",
    "sources.spotifySignInFailed": "La connexion à Spotify n'a pas abouti. Réessayez.",
    "sources.backupFailed": "La lecture de secours n'a pas pu être configurée. Vérifiez l'adresse de redirection et l'e-mail dans « User Management », puis réessayez.",
    "sources.spotifyChangeFailed": "Ce changement n'a pas pu être enregistré.",

    // Spotify: the banner at the top of the screen.
    "sources.bannerStopped": "Spotify ne joue plus pour le moment. Les autres sources marchent.",
    "sources.bannerRefused": "Ce compte Spotify a besoin de la lecture de secours.",
    "sources.bannerPremium": "Spotify demande un compte Premium pour écouter.",
    "sources.bannerShow": "Voir Spotify",

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

    // Order of preference.
    "sources.sectionPriority": "Ordre de préférence",
    "sources.priorityHint": "Un titre présent dans plusieurs sources se joue depuis la première. Alt+Haut et Alt+Bas déplacent la ligne choisie.",
    "sources.sourceOff": " (coupée)",
    "sources.moveUp": "Monter {name}",
    "sources.moveDown": "Descendre {name}",
    "sources.moved": "{name} est maintenant en position {position}.",
  },
  en: {
    "sources.title": "Sources",
    "sources.intro": "Each active source answers the search. Their order decides which one plays a track found in several.",
    "sources.settingNotSaved": "This setting could not be saved.",

    // Source cards.
    "sources.statusOff": "Off. Its results do not appear in search.",
    "sources.statusMissingTool": "On, but its component is not installed.",
    "sources.statusOn": "On. No account needed.",
    "sources.enableSource": "Enable the {name} source",
    "sources.copy": "Copy",
    "sources.copied": "Copied to the clipboard.",
    "sources.cancel": "Cancel",

    // YouTube.
    "sources.checkAgain": "Check again",
    "sources.componentInstalled": "YouTube component installed, version {version}.",
    "sources.componentMissing": "YouTube needs a component that is not installed on this computer.",
    "sources.installGuide": "The installation guide explains how to add it: ",

    // Spotify: the head of the card.
    "sources.spotifySignedOut": "On, but no account is connected.",
    "sources.spotifyInUnisson": "On. Music plays inside Unisson.",
    "sources.spotifyInApp": "On. Music plays in the Spotify application.",
    "sources.spotifyViaApp": "On. Music goes through the Spotify application for now.",
    "sources.spotifyStopped": "On, but playback is interrupted.",

    // Spotify: the account.
    "sources.spotifyAccount": "Account",
    "sources.spotifyNoAccount": "No account connected",
    "sources.spotifyPremiumNeeded": "A Spotify Premium account is needed.",
    "sources.spotifySignIn": "Sign in with Spotify…",
    "sources.spotifyWaiting": "Confirm the sign-in in your browser…",
    "sources.spotifyWaitingHint": "The Spotify page just opened. Did it not appear? Copy the link.",
    "sources.spotifyCopyLink": "Copy the link",
    "sources.spotifySignedIn": "Account connected",
    "sources.spotifySignedInHint": "Spotify Premium is needed to listen.",
    "sources.spotifySignOut": "Sign out",
    "sources.spotifyAbout": "Music plays inside Unisson, with its volume and its queue. No other application to open.",

    // Spotify: where it plays.
    "sources.spotifyPlayback": "Playback",
    "sources.chipInUnisson": "Inside Unisson",
    "sources.chipViaApp": "Through the Spotify application",
    "sources.chipStopped": "Interrupted",
    "sources.chipRefused": "Not with this account",
    "sources.chipPremium": "Premium needed",
    "sources.spotifyOkHint": "No other application to open.",
    "sources.spotifyAppOnlyHint": "Keep the Spotify application open. Sign in with Spotify to listen inside Unisson.",
    "sources.spotifyLimitedViaApp": "Spotify limits playback inside Unisson for now. Unisson tries again on its own {when}. Keep the Spotify application open.",
    "sources.spotifyRefusedViaApp": "Spotify does not let this account play its music inside Unisson. It goes through the Spotify application.",
    "sources.spotifyLimitedStopped": "Spotify refuses playback inside Unisson for now. Next try {when}. A backup would avoid this break.",
    "sources.spotifyRefusedStopped": "Spotify does not let this account play its music inside Unisson. This is common with accounts created since late 2024. The backup solves it.",
    "sources.spotifyPremiumStopped": "Spotify only lets Premium accounts listen.",
    "sources.retryIn.one": "in {count} minute",
    "sources.retryIn.other": "in {count} minutes",
    "sources.retrySoon": "at the next track",
    "sources.spotifyRetryNow": "Try again now",
    "sources.spotifyRetry": "Try again",
    "sources.spotifySetUpBackup": "Set up the backup…",

    // Spotify: the backup.
    "sources.backupTitle": "Backup playback",
    "sources.backupNone": "Optional. Not set up.",
    "sources.backupReady": "Ready. Used only when needed.",
    "sources.backupInUse": "Ready. In use right now.",
    "sources.backupIntro": "Useful when playback inside Unisson fails. It needs a Premium account and about 5 minutes. Music then goes through the Spotify application.",
    "sources.backupReadyIntro": "When playback inside Unisson fails, music goes through the Spotify application. It must be open on this computer.",
    "sources.backupApp": "Your Spotify app",
    "sources.backupTest": "Test",
    "sources.backupRemove": "Remove",
    "sources.backupStep1": "Create a Spotify app",
    "sources.backupStep1Detail": "On the developer site, give it a name and tick “Web API”.",
    "sources.backupOpenDashboard": "Open the developer site",
    "sources.backupStep2": "Add the redirect URI",
    "sources.backupStep3": "Allow your account",
    "sources.backupStep3Detail": "In “User Management”, add the email of this Spotify account. Without it, Spotify refuses the sign-in.",
    "sources.backupStep4": "Paste the Client ID of the app",
    "sources.backupClientIdPlaceholder": "32 letters and digits",
    "sources.backupSave": "Test and save",
    "sources.backupKeep": "You can leave this screen: what you typed stays here.",
    "sources.autoSwitch": "Switch automatically",
    "sources.autoSwitchDetail": "Only on known failures of playback inside Unisson. Unisson tells you each time.",
    "sources.autoSwitchUnavailable": "Available once the backup is ready.",

    // Spotify: what went wrong.
    "sources.spotifyInvalidId": "This Client ID is not valid. It has 32 letters and digits.",
    "sources.spotifyNetwork": "Spotify is not responding. Check your connection.",
    "sources.spotifySignInFailed": "The sign-in to Spotify did not work. Try again.",
    "sources.backupFailed": "The backup could not be set up. Check the redirect URI and the email in “User Management”, then try again.",
    "sources.spotifyChangeFailed": "This change could not be saved.",

    // Spotify: the banner at the top of the screen.
    "sources.bannerStopped": "Spotify does not play for now. The other sources work.",
    "sources.bannerRefused": "This Spotify account needs the backup.",
    "sources.bannerPremium": "Spotify needs a Premium account to listen.",
    "sources.bannerShow": "Show Spotify",

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

    // Order of preference.
    "sources.sectionPriority": "Order of preference",
    "sources.priorityHint": "A track found in several sources plays from the first one. Alt+Up and Alt+Down move the chosen row.",
    "sources.sourceOff": " (off)",
    "sources.moveUp": "Move {name} up",
    "sources.moveDown": "Move {name} down",
    "sources.moved": "{name} is now in position {position}.",
  },
};
