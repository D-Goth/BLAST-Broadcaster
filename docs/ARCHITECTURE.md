# Architecture

## Vue d'ensemble

```
┌──────────────────────────────────────────────────────────────────┐
│                    Interface (Tauri : HTML/CSS/JS)               │
│   platines · crossfader · soundbox · file · chat · OBS · thèmes  │
└───────────────▲──────────────────────────────────┬───────────────┘
                │ événements (état, VU, titre)     │ commandes
┌───────────────┴──────────────────────────────────▼───────────────┐
│                      Bus de commandes / événements               │
└──────┬─────────────┬───────────────┬──────────────┬──────────────┘
       │             │               │              │
┌──────▼─────┐ ┌─────▼──────┐ ┌──────▼──────┐ ┌─────▼─────┐ ┌──────────┐
│ blast-core │ │ blast-cast │ │blast-library│ │blast-aura │ │blast-live│
│ moteur     │─▶ encodeurs  │ │ bibliothèque│ │ OBS       │ │ chat     │
│ audio      │ │ Icecast/SC │ │ file, autoDJ│ │ websocket │ │ commandes│
└──────┬─────┘ └────────────┘ └─────────────┘ └───────────┘ └──────────┘
       │
   cpal : WASAPI / ASIO · CoreAudio · ALSA / JACK / PipeWire
```

## Principes

- **Le thread audio est sacré.** Il ne fait ni allocation, ni verrou bloquant, ni accès disque ou réseau.
  Il communique avec le reste via des files sans verrou (ring buffers). Une coupure d'antenne est le pire bug possible.
- **Un seul mix master**, distribué en parallèle vers la sortie locale, chaque encodeur de `blast-cast` et OBS.
- **La pré-écoute (PFL)** est un bus séparé, envoyé vers un autre périphérique de sortie.
- **L'interface ne possède aucun état audio** : elle envoie des commandes et affiche les événements reçus.
- **OBS est piloté, pas embarqué** : obs-websocket v5, aucune liaison avec libobs.

## Dépendances envisagées

| Besoin | Crate / bibliothèque | Licence |
|---|---|---|
| Sortie audio multi-OS | `cpal` | Apache-2.0 |
| Décodage MP3/AAC/FLAC/Vorbis/WAV | `symphonia` | MPL-2.0 |
| Encodage MP3 | LAME (liaison dynamique) | LGPL |
| Encodage Opus / Vorbis / FLAC | libopus / libvorbis / libFLAC | BSD |
| Rééchantillonnage | `rubato` | MIT |
| Interface | Tauri | MIT / Apache-2.0 |
| WebSocket (OBS, chat) | `tokio-tungstenite` | MIT |

## Règle de licence des dépendances

Le code étant sous PolyForm Noncommercial, **aucune dépendance GPL ne doit être liée au binaire**
(la GPL interdit d'ajouter une restriction non commerciale). LGPL en liaison dynamique, MIT, Apache, BSD et MPL sont acceptées.
FFmpeg n'est envisageable qu'en compilation LGPL, sans `--enable-gpl`.
L'encodage AAC (fdk-aac) a une licence de redistribution particulière : à traiter au jalon M9.

## Thèmes

Les thèmes sont des fichiers TOML dans `themes/`. L'interface n'utilise jamais de couleur en dur,
uniquement les tokens définis dans le thème, exposés en variables CSS (`--bg`, `--accent`, `--deck-a`…).
Les couleurs de mesure (`meter_*`) et d'alerte de saturation (`clip`) sont séparées de l'accent
pour rester lisibles quel que soit le thème.
