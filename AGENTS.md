# Workspace canonique obligatoire

- L'unique copie modifiable de WatchMind est `/home/e6/projects/watchmind` dans
  la distribution WSL `Ubuntu`.
- Avant toute mutation, exécuter `git rev-parse --show-toplevel`, puis vérifier
  que `$WSL_DISTRO_NAME` vaut `Ubuntu`.
- Si le dépôt courant se trouve sous `C:\`, `/mnt/c`, dans un autre clone ou
  dans un worktree secondaire, refuser toute modification, tout commit et toute
  génération de fichier. Les diagnostics en lecture seule restent permis.
- Demander alors à l'utilisateur de rouvrir Codex depuis
  `/home/e6/projects/watchmind` sous WSL.
- Ne pas créer de second clone ou worktree modifiable sans demande explicite de
  l'utilisateur et sans stratégie de suppression immédiate après usage.

# Consigne de vérification

WatchMind est un projet personnel. Garder les vérifications proportionnées au
risque : tests unitaires ou d'intégration ciblés, formatage et lint suffisent en
règle générale. Ne pas imposer de mutation testing, d'objectif de couverture,
de matrice exhaustive ni d'infrastructure de test de niveau professionnel sauf
demande explicite de l'utilisateur ou risque concret qui le justifie.
