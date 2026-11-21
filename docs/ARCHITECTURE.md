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

## Moteur audio (M1)

| Fichier | Rôle |
|---|---|
| `blast-core/src/decode.rs` | Ouverture d'un fichier, décodage en stéréo flottante, lecture des tags titre/artiste |
| `blast-core/src/resample.rs` | Rééchantillonnage sinc (rubato) vers la fréquence de la sortie |
| `blast-core/src/mixer.rs` | Mixeur temps réel : platines A/B, crossfader à puissance constante, auto-mix, événements |
| `blast-core/src/engine.rs` | Sortie cpal : thread audio, files de commandes et d'événements, récupération mémoire |

**Commandes** (`Command`) : `Load`, `Eject`, `Play`, `Pause`, `Stop`, `Seek`, `SetDeckGain`,
`SetCrossfader`, `StartCrossfade`, `SetMaster`, `SetAutoMix`.

**Événements** (`Event`) : `Ended`, `AutoTransition`, `CrossfadeDone`, et `Tick` dix fois par seconde
(état des platines, position du crossfader, crêtes du master). Les `Tick` passent par une file séparée :
s'ils s'accumulent, seul le plus récent est gardé, et ils ne peuvent jamais bloquer un événement important.

**Auto-mix** : quand la platine à l'antenne arrive à `fade_frames` de sa fin et que l'autre est chargée,
l'autre démarre et le crossfader glisse vers elle. Si la platine suivante est chargée trop tard
(fichier long à décoder après un jingle court), elle démarre dès qu'elle est prête.

**Mémoire** : le thread audio ne libère jamais une piste. Une piste remplacée est renvoyée par une file
« poubelle », vidée par `Engine::collect_garbage()` côté application.

**Limites actuelles** : chaque piste est décodée entièrement en mémoire (environ 23 Mo par minute à 48 kHz),
le master est simplement écrêté à ±1 (le limiteur arrive en M6), et symphonia 0.5 ne décode pas l'Opus.

## Application de bureau (M3a)

```
 app/ui (HTML/CSS/JS)  ── invoke("blast", {cmd}) ──▶  main.rs ──▶ file de commandes
        ▲                                                         │
        │ événements blast://state (10/s), blast://playlist,      ▼
        │ blast://notice                                   controller.rs (thread dédié)
        └──────────────────────────────────────────────── │  possède Engine, Playlist
                                                           ├─ threads de décodage
                                                           └─ threads de scan (dossiers, tags)
```

| Fichier | Rôle |
|---|---|
| `src-tauri/src/main.rs` | Démarrage Tauri, commandes `blast`, `themes`, `themes_dir`, fichiers passés en argument |
| `src-tauri/src/controller.rs` | Thread qui possède le moteur : traduit les commandes de l'interface, gère l'auto-mix, publie l'état |
| `src-tauri/src/playlist.rs` | Playlist (insertion, déplacement, retrait), parcours récursif des dossiers, lecture rapide des tags |
| `src-tauri/src/themes.rs` | Thèmes intégrés (`themes/*.toml` compilés dans le binaire) et thèmes utilisateur |
| `ui/app.js` | Rendu, glisser-déposer interne (pointeur) et depuis le système (événements Tauri), sélecteurs de fichiers |

**Commandes de l'interface** (`UiCommand`, JSON `{"type": …}`) : `refresh`, `add_paths`, `load_paths`, `remove`, `move`,
`clear`, `load_entry`, `play`, `pause`, `stop`, `eject`, `seek`, `crossfader`, `crossfade`, `auto_mix`, `fade_seconds`.

**Auto-mix côté application** : dès qu'une platine est libre (vide ou terminée) et que l'autre est occupée,
le contrôleur y charge le titre suivant de la playlist. Si les deux sont libres, il charge et lance la platine du côté du crossfader.

**Thèmes utilisateur** : fichiers `.toml` au même format que `themes/`, dans le dossier de configuration
(`~/.config/fr.black-lab.blast/themes` sous Linux). Le bouton **Thèmes…** crée ce dossier et l'affiche.

## Diffusion (M2)

```
 thread audio ──(tampon circulaire rtrb, 2 s)──▶ contrôleur ──Arc<[f32]>──▶ Caster
                                                                          ├─ sortie 1 : encodeur LAME → TCP (Icecast PUT)
                                                                          └─ sortie 2 : encodeur LAME → TCP (Shoutcast)
```

| Fichier | Rôle |
|---|---|
| `blast-cast/src/encoder.rs` | Encodage MP3 (LAME), stéréo ou mono, 32 à 320 kb/s |
| `blast-cast/src/protocol.rs` | Connexion source Icecast (`PUT` + `Expect: 100-continue`), Shoutcast v1 (mot de passe sur port+1), Shoutcast v2 (`motdepasse:#SID`), mise à jour du titre (`/admin/metadata`, `/admin.cgi`), test de connexion |
| `blast-cast/src/caster.rs` | Un thread par sortie : connexion, encodage, envoi, reconnexion toutes les 5 s, arrêt définitif sur mot de passe refusé |
| `blast-aura/src/lib.rs` | Test de connexion obs-websocket v5 (Hello, authentification SHA-256, GetVersion) |
| `app/src-tauri/src/config.rs` | Réglages en TOML (sans aucun mot de passe) |
| `app/src-tauri/src/secrets.rs` | Mots de passe dans le trousseau système (`keyring`), repli en mémoire si indisponible |

Le thread audio ne fait qu'écrire dans le tampon circulaire, sans verrou ni allocation. Si une sortie
est lente, ses blocs en attente sont abandonnés (128 blocs, environ 1,5 s) : elle ne peut jamais bloquer
le son local ni les autres sorties. Les valeurs envoyées dans les en-têtes sont nettoyées des retours à la ligne.

Testé contre Icecast 2.4.4 (sortie normale et port de compatibilité Shoutcast) : flux MP3 écoutable,
titre mis à jour au changement de morceau, reconnexion après redémarrage du serveur.
Shoutcast v2 (DNAS 2) n'a été testé que contre un serveur simulé.

## Voice FX et Sound FX (M5)

```
 micro (flux d'entrée cpal) ─▶ mono + rééchantillonnage linéaire ─▶ tampon rtrb ─▶ mixeur
 mixeur : (platines × crossfader × volume musique × ducking) + micro (coupe-bas, EQ, gain, rampe 20 ms) + pads
```

| Fichier | Rôle |
|---|---|
| `blast-core/src/dsp.rs` | Filtres biquad (coupe-bas, plateaux grave et aigu, cloche médium) |
| `blast-core/src/mixer.rs` | Jusqu'à 16 sons de pads simultanés, chaîne micro, ducking (descente 150 ms, remontée 600 ms) |
| `blast-core/src/engine.rs` | Flux d'entrée micro, retour des pistes terminées hors du thread audio |

Le retard du micro est borné : si le tampon dépasse 80 ms (horloges d'entrée et de sortie différentes),
l'excédent est abandonné. Le micro s'ouvre et se ferme avec une rampe de 20 ms pour éviter les clics.
Les sons des pads sont décodés en arrière-plan et gardés en mémoire.

## Dépendances envisagées

| Besoin | Crate / bibliothèque | Licence |
|---|---|---|
| Sortie audio multi-OS | `cpal` | Apache-2.0 |
| Décodage MP3/AAC/FLAC/Vorbis/WAV | `symphonia` | MPL-2.0 |
| Encodage Opus / Vorbis / FLAC | libopus / libvorbis / libFLAC | BSD |
| Rééchantillonnage | `rubato` | MIT |
| Interface | Tauri 2, tauri-plugin-dialog | MIT / Apache-2.0 |
| Thèmes | `toml`, `serde_json` | MIT / Apache-2.0 |
| WebSocket (OBS, chat) | `tokio-tungstenite` | MIT |
| Files temps réel | `crossbeam-channel`, `rtrb` | MIT / Apache-2.0 |
| Encodage MP3 | `mp3lame-encoder` (LAME compilé avec le projet) | LGPL (LAME), MIT |
| Trousseau | `keyring` | MIT / Apache-2.0 |
| OBS | `tungstenite`, `sha2`, `base64` | MIT / Apache-2.0 |
| Ligne de commande, WAV | `clap`, `hound` | MIT / Apache-2.0 |

## Règle de licence des dépendances

Le code étant sous PolyForm Noncommercial, **aucune dépendance GPL ne doit être liée au binaire**
(la GPL interdit d'ajouter une restriction non commerciale). LGPL en liaison dynamique, MIT, Apache, BSD et MPL sont acceptées.
FFmpeg n'est envisageable qu'en compilation LGPL, sans `--enable-gpl`.
LAME est compilé dans le binaire : c'est compatible avec la LGPL tant que le code source complet de BLAST
reste disponible, ce qui permet à chacun de recompiler avec une autre version de LAME.
L'encodage AAC (fdk-aac) a une licence de redistribution particulière : à traiter au jalon M9.

## Thèmes

Les thèmes sont des fichiers TOML dans `themes/`. L'interface n'utilise jamais de couleur en dur,
uniquement les tokens définis dans le thème, exposés en variables CSS (`--bg`, `--accent`, `--deck-a`…).
Les couleurs de mesure (`meter_*`) et d'alerte de saturation (`clip`) sont séparées de l'accent
pour rester lisibles quel que soit le thème.
