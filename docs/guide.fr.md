# Unisson en cinq minutes

*[Read this guide in English](guide.md)*

Unisson lit quatre choses dans une seule fenêtre : **les radios**, **vos
fichiers de musique**, **Spotify** et **YouTube**. Cette page montre le peu qu'il faut
savoir.

## 1. Installer

Téléchargez l'installateur sur la
[page des versions](https://github.com/geekabel/Unisson/releases).

Sous Ubuntu, ouvrez un terminal dans votre dossier de téléchargements et
tapez :

```sh
sudo apt install ./Unisson_0.1.0_amd64.deb
```

Ouvrez ensuite le menu des applications et lancez **Unisson**.

La radio et vos fichiers marchent tout de suite. YouTube a besoin d'un petit
programme en plus : le [README](../README.md#youtube) explique comment
l'ajouter. Spotify demande un compte Premium : ouvrez **Sources** et choisissez
**Se connecter avec Spotify**. Le [README](../README.md#spotify) en dit plus,
et explique la lecture de secours.

## 2. Écouter quelque chose

Cliquez dans le champ du haut, ou appuyez sur **Ctrl+K**. Tapez quelques
lettres : un titre, un artiste ou une radio. Appuyez sur **Entrée**. C'est
tout.

- **Entrée** lit tout de suite.
- **Maj+Entrée** ajoute à la file, pour plus tard.
- **Ctrl+Entrée** lit juste après le titre en cours.

Vous pouvez aussi y coller un lien : une adresse YouTube ou Spotify, ou
l'adresse d'un flux de radio.

## 3. Vos fichiers de musique

Unisson affiche la musique de votre dossier **Musique** sur l'accueil. Copiez-y
un fichier : il apparaît quelques secondes plus tard.

Votre musique est ailleurs ? Ouvrez **Sources**, puis choisissez **Changer de
dossier**.

## 4. La radio

Ouvrez **Radios**. Cherchez une station par son nom, ou choisissez un genre et
un pays. Cliquez sur une station pour l'écouter.

Cliquez sur le **cœur**, en bas de la fenêtre, pour garder une station. Vos
stations vous attendent ensuite sur l'accueil.

## 5. La file d'attente

Le panneau de droite liste ce qui sera lu ensuite. Vous pouvez monter ou
descendre un titre, le retirer, ou cliquer dessus pour le lire tout de suite.

Quand la file est vide, votre dernière radio reprend toute seule.
L'interrupteur en bas du panneau coupe ce comportement.

## 6. Garder ce que vous aimez

- Le **cœur** garde un titre ou une station dans les **Favoris**.
- L'**Historique** liste ce que vous avez écouté.
- **Nouvelle liste**, dans le menu de gauche, crée une liste de lecture. Le
  bouton avec un petit plus, sur chaque ligne, y ajoute un titre.

## 7. Écouter en travaillant

- **Fermez la fenêtre** : la musique continue. L'icône d'Unisson reste dans la
  barre du haut de l'écran. Cliquez dessus pour mettre en pause, changer de
  radio ou quitter.
- **Les touches de votre clavier** : lecture, suivant et précédent marchent
  depuis n'importe quelle application.
- **Le mini-lecteur** : le bouton en bas à droite réduit Unisson à une petite
  barre.
- **Le lanceur rapide** : une petite fenêtre de recherche qui s'ouvre
  par-dessus votre travail. Dans **Réglages**, choisissez un raccourci et
  cliquez sur **Installer**. Ensuite, depuis n'importe où : le
  raccourci, quelques lettres, Entrée.

## 8. Régler à votre goût

Dans **Réglages** :

- **Thème** : sombre, clair, ou comme votre système.
- **Langue** : français, anglais, ou comme votre système.
- Coupez les notifications.

Dans **Sources**, coupez une source que vous n'utilisez pas, ou changez leur
ordre.

Le texte est trop petit ? Appuyez sur **Ctrl** et **+**. **Ctrl+0** revient à
la taille normale.

## Les touches utiles

| Touches | Action |
| --- | --- |
| Espace | Lire ou mettre en pause |
| Ctrl+K | Rechercher |
| Ctrl+Droite, Ctrl+Gauche | Suivant, précédent |
| Ctrl+Haut, Ctrl+Bas | Plus fort, moins fort |
| Alt+1 à Alt+6 | Accueil, Radios, Favoris, Historique, Sources, Réglages |
| F1 | Tous les raccourcis clavier |

## Si quelque chose ne marche pas

- **Une radio ne démarre pas.** La station est peut-être fermée ou saturée.
  Cliquez sur **Réessayer**, ou choisissez-en une autre.
- **« Le réseau ne répond pas. »** Vérifiez votre connexion, puis cliquez sur
  **Réessayer**.
- **YouTube ne donne aucun résultat.** Ouvrez **Sources** et regardez la carte
  YouTube. Si son composant manque ou est trop ancien, le
  [README](../README.md#youtube) dit quoi faire.
- **Un titre Spotify ne se lance pas.** Ouvrez **Sources** et regardez la
  carte Spotify : elle dit pourquoi. Spotify demande un compte Premium. Quand
  Spotify refuse la lecture dans Unisson, la lecture de secours par
  l'application Spotify règle le problème.
- **Un titre de mon dossier manque.** Ouvrez **Sources** et cliquez sur
  **Analyser à nouveau**.
- **Le raccourci du lanceur ne fait rien.** Un autre programme utilise
  peut-être les mêmes touches. Choisissez un autre raccourci dans **Réglages**.

Toujours bloqué ? [Dites-le-nous](https://github.com/geekabel/Unisson/issues).
Laissez la souris sur le message apparu : une petite bulle affiche un détail
technique qui aide beaucoup.
