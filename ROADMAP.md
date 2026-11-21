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
| **M2b** | Compteurs façon SAM dans la barre du haut : auditeurs en cours, pic d'auditeurs (max peak), durée de diffusion (timer du stream), par sortie et au total | Suivre son audience en direct |
| **M3b** | Interface : forme d'onde, raccourcis clavier, préférences (sortie audio, dossier par défaut), sauvegarde de la playlist | Confort d'utilisation au quotidien |
| **M4** | Bibliothèque : import, tags, pochettes, file d'attente, auto-DJ simple | Laisser tourner la radio sans intervention |
| **M5** | **Voice FX** (micro) et **Sound FX** (soundboard), pré-écoute sur sortie casque — voir ci-dessous | Animer une émission en direct |
| **M5b** | Platines façon SAM : volume et VU par platine, boutons Air/Cue, pitch, pochette, format du fichier ; file d'attente et historique séparés ; dispositions d'écran (bureaux A/B/C) | Parité avec SAM Broadcaster |
| **M6** | Traitement audio master : AGC, compresseur, limiteur, normalisation EBU R128 | Un « son radio » homogène |
| **M7** | Aura : connexion obs-websocket, scènes, stream, Now Playing vers OBS | Lancer le live vidéo depuis BLAST |
| **M8** | Live : chat Twitch puis YouTube et Kick, commandes viewers, bot Now Playing | Interaction avec le public |
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

**Voice FX (micro)**
- Entrée micro choisie dans Réglages → Audio, avec son propre volume et son VU-mètre.
- **Mic On** : bascule marche/arrêt.
- **Appuyer pour parler** : micro ouvert tant que le bouton, ou une touche de raccourci, est maintenu.
- **Auto** (ducking) : la musique baisse automatiquement quand le micro est ouvert, avec niveau et vitesse réglables.
- Égaliseur du micro (graves, médiums, aigus, filtre coupe-bas), noise gate, compresseur voix.
- Faders séparés Musique / Micro.

**Sound FX (soundboard)**
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
