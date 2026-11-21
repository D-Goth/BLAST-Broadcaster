# BLAST Broadcaster

**Black-Lab Audio Streaming Tool** — console de webradio et de live multi-OS (Linux, Windows, macOS).

BLAST Broadcaster réunit dans un seul logiciel ce qu'il fallait jusqu'ici assembler à la main :
la console d'antenne d'une webradio (à la manière de SAM Broadcaster), la diffusion vers les serveurs
de streaming audio, et le pilotage d'un live vidéo avec webcams et chat (talk show, live DJ, émission).

> Statut : **squelette (M0)**. Le projet est en cours de conception, rien n'est encore utilisable en antenne.

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

L'interface (Tauri) arrivera dans `app/` au jalon M3. Voir [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) et [ROADMAP.md](ROADMAP.md).

## Compiler

Prérequis : Rust stable (1.75 ou plus récent).

```bash
cargo build
cargo test
```

## Licence

- **Code** : [PolyForm Noncommercial 1.0.0](LICENSE) — libre pour tout usage non commercial (personnel, associatif, éducatif, recherche).
- **Ressources** (thèmes, documentation, visuels) : [CC BY-NC 4.0](LICENSE-ASSETS).
- **Nom et logo** : non couverts par ces licences, voir [TRADEMARK.md](TRADEMARK.md).

Pour un usage commercial, contacter l'auteur.

---

© 2026 Loïc HERAUDEAU (D-Goth) — [black-lab.fr](https://black-lab.fr)
