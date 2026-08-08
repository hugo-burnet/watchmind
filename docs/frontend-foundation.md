# Direction visuelle et socle frontend

## Thèse

WatchMind est une table de montage du goût, destinée à un cinéphile ou
animephile auto-hébergeur. La page doit permettre de comprendre rapidement ce
qui relie une recommandation aux œuvres aimées.

La carte de goût est le seul geste visuel fort : chemins, repères et contours
traduisent les proximités. Navigation, formulaires et états restent calmes pour
ne pas concurrencer la décision.

## Tokens

| Rôle | Valeur |
| --- | --- |
| Encre | `#25243A` |
| Papier froid | `#F7F8FC` |
| Brume cartographique | `#E9EDF6` |
| Affinité | `#6259B7` |
| Tension / pari | `#E36F61` |
| Confiance | `#3F9EAA` |
| Titres | Bahnschrift Condensed / Arial Narrow |
| Lecture | Charter / Cambria |
| Données | IBM Plex Mono / Cascadia Mono |

Les espaces suivent six paliers de `0.375rem` à `4.5rem`. Les surfaces sont
rectilignes ; les formes organiques sont réservées aux contours de la carte.

## Structure responsive

```text
Desktop                         Mobile
┌────────┬──────────────────┐   ┌──────────────────┐
│ marque │ titre + action   │   │ marque / onglets │
│        ├──────────────────┤   ├──────────────────┤
│ nav    │ carte de goût    │   │ titre            │
│        │  sources → cible │   ├──────────────────┤
│ état   ├──────────────────┤   │ carte verticale  │
│        │ états système    │   ├──────────────────┤
└────────┴──────────────────┘   │ états empilés    │
                               └──────────────────┘
```

Le focus clavier utilise un contour corail de trois pixels. Un lien d'évitement
précède le shell. Les animations de trajet et de chargement sont neutralisées
avec `prefers-reduced-motion`.

## Revue visuelle

Les captures du laboratoire ont été vérifiées aux formats desktop 1440 px et
mobile 390 px : la carte se transforme en parcours vertical, la navigation
devient horizontale et les états s'empilent sans débordement de page.

- [Capture desktop](screenshots/watchmind-l15-desktop.png)
- [Capture mobile](screenshots/watchmind-l15-mobile.png)

Pour les régénérer, lancer `npm run preview`, puis `npm run capture` depuis
`apps/watchmind-web` avec les navigateurs Playwright installés.

Le lot 16 ajoute les captures du parcours bibliothèque, sans remplacer celles
du laboratoire : `watchmind-l16-desktop.png` et `watchmind-l16-mobile.png`.

Les lots 17 et 18 prolongent cette revue avec les vues explicables « Pour
vous » et « Profil de goût », chacune capturée en desktop et mobile sous
`watchmind-l17-*` et `watchmind-l18-*`.
