# Feuille de route

Chaque jalon doit produire quelque chose d'utilisable avant de passer au suivant.
L'interface (M3a) a été avancée avant la diffusion (M2) pour que la suite s'adapte à l'usage réel.

| Jalon | Contenu | Résultat attendu |
|---|---|---|
| **M0** | Squelette du dépôt, modules, licences, thèmes | Le workspace compile, les tests passent ✅ |
| **M1** | Moteur audio : décodage (symphonia), 2 platines, crossfade, sortie locale (cpal) | Enchaîner deux morceaux en fondu sur les enceintes ✅ |
| **M1.5** | Décodage en flux (au lieu de tout charger en mémoire), lecture Opus, détection des silences de fin | Pistes longues (mixes d'une heure) sans surconsommation mémoire |
| **M3a** | Interface Tauri v1 : platines, crossfader, auto-mix, VU-mètres, thèmes, playlist, glisser-déposer et sélecteur de fichiers, icône et lanceur | Piloter le moteur à la souris ✅ |
| **M2** | Cast : encodage MP3 (LAME), Icecast, Shoutcast v1/v2, titre en cours, reconnexion automatique, panneau Réglages (Diffusion, Aura, Audio), mots de passe dans le trousseau | Une webradio écoutable dans un navigateur ✅ |
| **M2b** | Compteurs façon SAM dans la barre du haut : auditeurs en cours, pic d'auditeurs (max peak), durée de diffusion (timer du stream), par sortie et au total | Suivre son audience en direct ✅ |
| **M2c** | Serveur Icecast intégré : profils local et VPS, détection de l'installation, Démarrer / Arrêter, configuration `icecast.xml` par formulaires avec sauvegardes, mountpoints et secours, tableau de bord (auditeurs, pic, débit, graphique), auditeurs, actions admin, journaux | Ne plus jamais éditer icecast.xml à la main ✅ |
| **M2d** | Icecast sur VPS par SSH : configuration, démarrage et journaux à distance, clé SSH dans le trousseau | Administrer le serveur de production depuis BLAST |
| **M3b** | Interface : forme d'onde, raccourcis clavier, préférences (sortie audio, dossier par défaut), sauvegarde de la playlist | Confort d'utilisation au quotidien |
| **M4** | Pilier 1 — Bibliothèque SQLite (titres et tags), table `historique_diffusion`, règles de séparation artiste/titre appliquées par l'auto-mix, purge automatique, onglet Historique et export CSV | Laisser tourner la radio sans répétitions ✅ |
| **M4c** | Historique typé (morceau, jingle, pub), tampons mémoire global et par catégorie, filtres, export par catégorie et période, box à jingles, interface de stockage prête pour MariaDB / MySQL | Bilans et déclarations par catégorie ✅ |
| **M4e** | Pilier 2 — Grilles horaires : bibliothèque scannée (BPM inclus), catégories à règles (genres, années, BPM, dossiers, type), grilles jouées en boucle, planning 7 × 24, tirage pondéré par l'ancienneté, remplissage de la playlist par l'auto-mix | Une antenne programmée heure par heure ✅ |
| **M4d** | Pilote MariaDB / MySQL (base externe, plusieurs postes sur une même radio), correction du type titre par titre | Travailler à plusieurs, ou en déplacement |
| **M4b** | Pilier 2 — Clockwheels : catégories, grilles horaires, sélection pondérée (genre, année, BPM) dans la bibliothèque | Programmer une grille d'antenne |
| **M5** | **Voice FX** (micro) et **Sound FX** (soundboard) — voir ci-dessous. Reste : noise gate, compresseur voix, modes boucle/maintien des pads, couleurs de pads, pré-écoute casque | Animer une émission en direct ✅ (base) |
| **M5b** | Platines façon SAM : volume et VU par platine, boutons Air/Cue, pitch, pochette, format du fichier ; file d'attente et historique séparés ; dispositions d'écran (bureaux A/B/C) | Parité avec SAM Broadcaster |
| **M6** | Pilier 3 — Traitement antenne (AGC, compresseur, limiteur à anticipation, presets Doux / Radio FM / Puissant) et boîte à effets de voix (Naturel, Podcast, Radio FM, ASMR, TSF 1930, Téléphone, Mégaphone) | Un « son radio » homogène ✅ |
| **M6b** | Compresseur multibande, normalisation EBU R128 des titres à l'import, presets voix personnalisables | Finitions du son |
| **M7** | Aura (repris du prototype Aura Control) : connexion obs-websocket avec reconnexion, scènes, stream, enregistrement et pause, replay, capture, volume et mute des sources, minuteurs, Now Playing vers une source texte OBS | Lancer le live vidéo depuis BLAST ✅ |
| **M7b** | Sortie « antenne » vers OBS (micro inclus) via une sortie audio virtuelle, raccourcis clavier OBS configurables | OBS diffuse exactement ce qui part à l'antenne |
| **M8** | Live : chat Twitch (lecture anonyme, jeton seulement pour écrire) ✅, commandes viewers (`!np`, `!request` avec file à valider, `!sfx`) ✅, bot Now Playing ✅ ; YouTube et Kick à venir | Interaction avec le public |
| **M9** | Formats supplémentaires : Opus, Vorbis, AAC | Plusieurs formats en parallèle |
| **M10** | Rotations avancées, planificateur, voice tracking, historique exportable | Programmation complète d'une grille |
| **M11** | Packaging Linux (AppImage, .deb, Flatpak), Windows (MSI), macOS (DMG) | Installation en un clic |

## Gestion des fichiers dans l'interface (M3a ✅)

- Glisser-déposer depuis l'explorateur de fichiers (Dolphin, Explorateur Windows, Finder) :
  - sur la **playlist** : ajout à la fin, ou à l'endroit du dépôt ;
  - sur une **platine** : chargement direct dans la platine.
- Glisser-déposer interne : réordonner la playlist, envoyer un titre de la playlist vers une platine.
- Dépôt d'un **dossier** : ajout de tous les fichiers audio qu'il contient, sous-dossiers compris.
- Bouton **Ajouter** ouvrant le sélecteur de fichiers natif, avec sélection multiple, et **Ajouter un dossier**.
- Fichiers non lisibles signalés dans la playlist plutôt que refusés en silence.

## Voice FX et Sound FX (M5)

Comparaison avec SAM Broadcaster 4.3.6 : ce sont les deux panneaux qui manquent pour l'animation en direct.

**Voice FX (micro)** — ✅ entrée micro, Mic On, appuyer pour parler (bouton et Espace), ducking AUTO, égaliseur 3 bandes + coupe-bas, faders Micro/Musique
- Entrée micro choisie dans Réglages → Audio, avec son propre volume et son VU-mètre.
- **Mic On** : bascule marche/arrêt.
- **Appuyer pour parler** : micro ouvert tant que le bouton, ou une touche de raccourci, est maintenu.
- **Auto** (ducking) : la musique baisse automatiquement quand le micro est ouvert, avec niveau et vitesse réglables.
- Égaliseur du micro (graves, médiums, aigus, filtre coupe-bas), noise gate, compresseur voix.
- Faders séparés Musique / Micro.

**Sound FX (soundboard)** — ✅ 36 pads en 3 pages, F1–F12, glisser-déposer, renommer, STOP, volume
- Fenêtre de pads, organisée en pages (1/3, 2/3…), chaque pad avec nom, couleur, raccourci clavier et volume.
- Glisser-déposer de fichiers sur un pad pour l'assigner ; banque de sons libres de droits fournie à part.
- Modes de lecture par pad : une fois, en boucle, tant que le pad est maintenu.
- Volume général, égaliseur, bouton **STOP** qui coupe tous les sons, bouton **Air/Cue** pour pré-écouter au casque.
- Pads pilotables plus tard depuis le chat (`!sfx`) et par MIDI.

## Panneau de configuration (M2 ✅ et M7)

Fenêtre **Réglages** à onglets, ouverte depuis la barre du haut :

- **Diffusion** (M2 ✅) : liste de sorties, chacune avec type de serveur (Icecast, Shoutcast v1, Shoutcast v2),
  hôte, port, point de montage ou SID, utilisateur, mot de passe, codec, débit, fréquence, canaux,
  nom et genre de la station. Boutons **Tester la connexion**, **Activer/Désactiver**, voyant d'état par sortie.
- **Aura / OBS** : hôte, port (4455 par défaut), mot de passe obs-websocket et test de connexion ✅ ;
  connexion automatique, choix des scènes et source texte « Now Playing » utilisés à partir de M7.
- **Audio** : sortie principale ✅ ; sortie casque (pré-écoute) et entrée micro avec M5.
- **Général** : dossier de musique par défaut, durée de fondu, thème.

Les mots de passe ne sont jamais écrits en clair dans les fichiers de configuration :
ils sont rangés dans le trousseau du système (KWallet ou Secret Service sous Linux,
Gestionnaire d'identifiants sous Windows, Trousseau sous macOS). Le reste des réglages est un fichier TOML lisible.

Idées pour plus tard : invités à distance en WebRTC avec mix-minus, contrôleurs MIDI, mode rainbow synchronisé au BPM, API locale pour plugins.
