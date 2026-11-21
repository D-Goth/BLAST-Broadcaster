# Feuille de route

Chaque jalon doit produire quelque chose d'utilisable avant de passer au suivant.
L'interface (M3a) a été avancée avant la diffusion (M2) pour que la suite s'adapte à l'usage réel.

| Jalon | Contenu | Résultat attendu |
|---|---|---|
| **M0** | Squelette du dépôt, modules, licences, thèmes | Le workspace compile, les tests passent ✅ |
| **M1** | Moteur audio : décodage (symphonia), 2 platines, crossfade, sortie locale (cpal) | Enchaîner deux morceaux en fondu sur les enceintes ✅ |
| **M1.5** | Décodage en flux (au lieu de tout charger en mémoire), lecture Opus, détection des silences de fin | Pistes longues (mixes d'une heure) sans surconsommation mémoire |
| **M3a** | Interface Tauri v1 : platines, crossfader, auto-mix, VU-mètres, thèmes, playlist, glisser-déposer et sélecteur de fichiers, icône et lanceur | Piloter le moteur à la souris ✅ |
| **M2** | Cast : encodage MP3 (LAME), connexion Icecast, métadonnées, panneau de diffusion dans l'interface | Une webradio écoutable dans un navigateur |
| **M3b** | Interface : forme d'onde, raccourcis clavier, préférences (sortie audio, dossier par défaut), sauvegarde de la playlist | Confort d'utilisation au quotidien |
| **M4** | Bibliothèque : import, tags, pochettes, file d'attente, auto-DJ simple | Laisser tourner la radio sans intervention |
| **M5** | Soundbox, micro avec ducking, pré-écoute PFL sur sortie casque | Animer une émission en direct |
| **M6** | Traitement audio master : AGC, compresseur, limiteur, normalisation EBU R128 | Un « son radio » homogène |
| **M7** | Aura : connexion obs-websocket, scènes, stream, Now Playing vers OBS | Lancer le live vidéo depuis BLAST |
| **M8** | Live : chat Twitch puis YouTube et Kick, commandes viewers, bot Now Playing | Interaction avec le public |
| **M9** | Multi-sorties : Opus, Vorbis, FLAC, AAC, Shoutcast v1/v2 | Plusieurs flux en parallèle |
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

Idées pour plus tard : invités à distance en WebRTC avec mix-minus, contrôleurs MIDI, mode rainbow synchronisé au BPM, API locale pour plugins.
