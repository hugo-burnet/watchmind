# CLI V1

`watchmind-cli` expose le moteur complet sans serveur ni persistance. Toutes les
commandes lisent un CSV de notes et un snapshot catalogue local.

| Commande | Résultat |
|---|---|
| `import-csv` | Valide les fixtures et affiche leur résumé. |
| `build-profile` | Construit le profil complet. |
| `show-poles` | Affiche les pôles, tags et œuvres représentatives. |
| `recommend` | Filtre, score et diversifie les candidats. |
| `explain <work-id>` | Recalcule le score et ses contributions pour une œuvre. |
| `evaluate` | Produit le rapport Markdown ou JSON et applique le verrou. |
| `leave-one-out` | Expose les métriques et rangs individuels du moteur. |
| `compare-baselines` | Compare les trois baselines historiques. |

Les commandes qui produisent des données structurées acceptent `--json`.
`evaluate` accepte en plus `--config <evaluation.json>` ; sans ce paramètre, la
configuration par défaut n'ajoute ni paire de régression ni dates temporelles.
En sortie texte, `evaluate` émet directement un rapport Markdown. Elle retourne
le code `2` si un seuil ou une paire de régression échoue.

La commande `help` imprime les formes exactes des huit commandes. Les chemins
peuvent être relatifs ou absolus et aucun accès réseau n'est effectué.
