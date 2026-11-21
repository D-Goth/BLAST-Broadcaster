# Feuille de route

Chaque jalon doit produire quelque chose d'utilisable avant de passer au suivant.

| Jalon | Contenu | Résultat attendu |
|---|---|---|
| **M0** | Squelette du dépôt, modules, licences, thèmes | Le workspace compile, les tests passent ✅ |
| **M1** | Moteur audio : décodage (symphonia), 2 platines, crossfade, sortie locale (cpal) | Enchaîner deux morceaux en fondu sur les enceintes |
| **M2** | Cast : encodage MP3 (LAME), connexion Icecast, métadonnées | Une webradio écoutable dans un navigateur |
| **M3** | Interface Tauri : platines, crossfader, VU-mètres, système de thèmes | Piloter M1 et M2 à la souris |
| **M4** | Bibliothèque : import, tags, pochettes, file d'attente, auto-DJ simple | Laisser tourner la radio sans intervention |
| **M5** | Soundbox, micro avec ducking, pré-écoute PFL sur sortie casque | Animer une émission en direct |
| **M6** | Traitement audio master : AGC, compresseur, limiteur, normalisation EBU R128 | Un « son radio » homogène |
| **M7** | Aura : connexion obs-websocket, scènes, stream, Now Playing vers OBS | Lancer le live vidéo depuis BLAST |
| **M8** | Live : chat Twitch puis YouTube et Kick, commandes viewers, bot Now Playing | Interaction avec le public |
| **M9** | Multi-sorties : Opus, Vorbis, FLAC, AAC, Shoutcast v1/v2 | Plusieurs flux en parallèle |
| **M10** | Rotations avancées, planificateur, voice tracking, historique exportable | Programmation complète d'une grille |
| **M11** | Packaging Linux (AppImage, .deb, Flatpak), Windows (MSI), macOS (DMG) | Installation en un clic |

Idées pour plus tard : invités à distance en WebRTC avec mix-minus, contrôleurs MIDI, mode rainbow synchronisé au BPM, API locale pour plugins.
