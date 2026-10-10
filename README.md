# Unisson

Lecteur audio unifié pour Linux, écrit en Rust.
Une seule recherche et une seule file d'attente pour la radio, les fichiers locaux et YouTube.

Le projet est en cours de construction. Cible actuelle : Ubuntu 22.04.

## Organisation du dépôt

- `crates/unisson-core` : le cœur, sans interface (modèle commun, sources, file d'attente, moteur audio).
- L'application graphique sera ajoutée dans une autre crate.

## Commandes utiles

```sh
cargo fmt --all            # mettre le code en forme
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace     # lancer les tests
```

## Méthode de travail

1. Pour le langage, on se fie à la documentation officielle de Rust.
2. Pour chaque bibliothèque, on lit sa documentation avant d'écrire du code.
3. Si un problème résiste, on cherche dans les tickets GitHub et les forums.
4. Une tâche à la fois, suivie dans le tableau Kanban du projet.
