/**
 * The keyboard shortcuts of the main window, as the user reads them.
 *
 * This list is only text: `shortcuts.js` and `search-box.js` hold the
 * behavior. It lives apart so that the settings screen can show it without
 * loading them.
 */

/**
 * Groups of shortcuts. Each shortcut has its keys, one array per
 * alternative, and what it does.
 *
 * @type {{title: string, items: {keys: string[][], text: string}[]}[]}
 */
export const SHORTCUT_GROUPS = [
  {
    title: "Lecture",
    items: [
      { keys: [["Espace"]], text: "Lire ou mettre en pause" },
      { keys: [["Ctrl", "→"]], text: "Titre suivant" },
      { keys: [["Ctrl", "←"]], text: "Titre précédent, ou retour au début" },
      { keys: [["Ctrl", "↑"], ["Ctrl", "↓"]], text: "Monter ou baisser le volume" },
    ],
  },
  {
    title: "Se déplacer",
    items: [
      { keys: [["Ctrl", "K"], ["/"]], text: "Aller au champ de recherche" },
      { keys: [["Alt", "1"], ["…"], ["Alt", "5"]], text: "Accueil, Radios, Favoris, Historique, Sources" },
      { keys: [["F6"], ["Maj", "F6"]], text: "Passer d'une zone à l'autre : menu, recherche, contenu, file, lecteur" },
      { keys: [["Tab"], ["Maj", "Tab"]], text: "Passer d'un élément au suivant, ou au précédent" },
      { keys: [["Ctrl", "+"], ["Ctrl", "-"], ["Ctrl", "0"]], text: "Agrandir ou réduire l'affichage, ou revenir à la taille normale" },
    ],
  },
  {
    title: "Dans la recherche",
    items: [
      { keys: [["↑"], ["↓"]], text: "Choisir un résultat" },
      { keys: [["Entrée"]], text: "Lire tout de suite" },
      { keys: [["Maj", "Entrée"]], text: "Ajouter à la file" },
      { keys: [["Ctrl", "Entrée"]], text: "Lire juste après le titre en cours" },
      { keys: [["Échap"]], text: "Effacer la recherche" },
    ],
  },
];
