# BLAST Broadcaster

**Black-Lab Audio Streaming Tool** — console de webradio et de live multi-OS (Linux, Windows, macOS).

BLAST Broadcaster réunit dans un seul logiciel ce qu'il fallait jusqu'ici assembler à la main :
la console d'antenne d'une webradio (à la manière de SAM Broadcaster), la diffusion vers les serveurs
de streaming audio, et le pilotage d'un live vidéo avec webcams et chat (talk show, live DJ, émission).

![BLAST Broadcaster — thème Black-Lab](docs/img/interface-black-lab.png)

> Statut : **interface v1**. Deux platines, crossfader, auto-mix, playlist avec glisser-déposer, thèmes. Pas encore de diffusion vers un serveur.

## Fonctionnalités prévues

**Antenne**
- Platines A/B (jusqu'à 4), crossfade automatique ou manuel, cue points, pré-écoute (PFL) sur sortie casque séparée
- Soundbox (cart wall) : jingles, virgules, effets, sur raccourcis clavier ou contrôleur MIDI
- File d'attente éditable en direct, auto-DJ avec rotations (clockwheels) et règles de séparation
- Micro avec ducking automatique, traitement audio (AGC, compresseur, limiteur, égaliseur)
- Voice tracking

**Diffusion**
- Plusieurs sorties simultanées : MP3, AAC, Opus, Ogg Vorbis, FLAC
- Serveurs Icecast, Shoutcast v1 et v2, métadonnées « titre en cours »
- Historique de diffusion exportable

**Live**
- Pilotage complet d'OBS Studio via obs-websocket (module Aura)
- Chat agrégé Twitch, YouTube, Kick, avec commandes viewers (`!request`, `!np`, `!sfx`)
- Overlay « Now Playing » automatique

**Interface**
- Thèmes personnalisables et partageables (Light, Dark et Black-Lab fournis)
- Couleur par platine, mode rainbow optionnel synchronisé au BPM

## Modules

| Crate | Rôle |
|---|---|
| `blast-core` | Moteur audio : platines, mixage, crossfade, traitement |
| `blast-cast` | Encodeurs et envoi vers les serveurs de streaming |
| `blast-library` | Bibliothèque musicale, file d'attente, automatisation |
| `blast-aura` | Pilotage d'OBS Studio |
| `blast-live` | Chat, commandes viewers, invités |
| `blast-cli` | Outil `blast` en ligne de commande pour tester le moteur |
| `app/src-tauri` | Application de bureau (Tauri) : contrôleur, playlist, thèmes |
| `app/ui` | Interface HTML/CSS/JS |

Voir [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) et [ROADMAP.md](ROADMAP.md).

## Installer sous Linux (Kubuntu, Ubuntu, Debian)

```bash
sudo apt install build-essential pkg-config curl git \
    libasound2-dev libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libsoup-3.0-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

git clone https://github.com/D-Goth/BLAST-Broadcaster.git
cd BLAST-Broadcaster
./scripts/install-linux.sh
```

Le script compile, puis installe pour l'utilisateur courant (sans `sudo`) :
- **BLAST Broadcaster** dans le menu des applications (Multimédia), avec son icône ;
- la commande `blast-broadcaster` (et l'outil de test `blast`) dans `~/.local/bin`.

Pour mettre à jour : `git pull` puis relancer le script. Pour désinstaller : `./scripts/install-linux.sh --uninstall`.

Sous Fedora : `sudo dnf install alsa-lib-devel webkit2gtk4.1-devel gtk3-devel librsvg2-devel libsoup3-devel`.
PipeWire et PulseAudio sont pris en charge via leur couche de compatibilité ALSA.

## Utiliser l'interface

| Action | Comment |
|---|---|
| Ajouter des titres | **+ Fichiers** (sélection multiple), **+ Dossier** (sous-dossiers compris), ou glisser depuis Dolphin sur la playlist |
| Charger une platine | Glisser un titre (ou un fichier depuis Dolphin) sur la platine, boutons **A** / **B** de la ligne, ou double-clic (platine libre) |
| Réordonner | Glisser un titre dans la playlist |
| Enchaîner | **FONDU → A/B** démarre l'autre platine et fond vers elle, puis arrête la précédente |
| Auto-mix | Pioche dans la playlist et enchaîne tout seul, avec la durée de fondu choisie (zone hachurée sur la barre de progression) |
| Se déplacer | Cliquer sur la barre de progression |
| Thèmes | Sélecteur en haut à droite ; **Thèmes…** indique le dossier où déposer tes propres fichiers `.toml` |

Un titre chargé mais pas encore joué, remplacé par un autre, retourne en tête de playlist.
Les fichiers illisibles restent dans la playlist, marqués ⚠, et sont sautés par l'auto-mix.

## Développer

```bash
cargo build --release
cargo test
./target/release/blast-broadcaster ~/Musique
```

## Outil de test en ligne de commande

```bash
./target/release/blast devices
./target/release/blast info ~/Musique/*.mp3
./target/release/blast play titre1.mp3 titre2.flac jingle.ogg --fade 6
./target/release/blast render titre1.mp3 titre2.flac -o mix.wav --fade 6
```

| Commande | Rôle |
|---|---|
| `devices` | Liste les sorties audio (`*` = sortie par défaut) |
| `info` | Durée, fréquence d'origine et tags de chaque fichier |
| `play` | Joue la liste en alternant les platines A et B, avec fondu enchaîné automatique |
| `render` | Calcule le même mix hors-ligne et l'écrit en WAV, pour vérifier sans enceintes |

Options utiles : `--fade <secondes>`, `--device "<nom>"` pour `play`, `--rate` et `--pcm16` pour `render`.

Formats lus : MP3, AAC/M4A, ALAC, FLAC, Ogg Vorbis, WAV, AIFF, MKV/WebM audio. Les fichiers sont rééchantillonnés à la fréquence de la sortie.

## Licence

- **Code** : [PolyForm Noncommercial 1.0.0](LICENSE) — libre pour tout usage non commercial (personnel, associatif, éducatif, recherche).
- **Ressources** (thèmes, documentation, visuels) : [CC BY-NC 4.0](LICENSE-ASSETS).
- **Nom et logo** : non couverts par ces licences, voir [TRADEMARK.md](TRADEMARK.md).

Pour un usage commercial, contacter l'auteur.

---

© 2026 Loïc HERAUDEAU (D-Goth) — [black-lab.fr](https://black-lab.fr)
