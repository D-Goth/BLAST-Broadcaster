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
 app/ui (HTML/CSS/JS)  ── invoke("blast", {cmd}) ──▶  api.rs ──▶ file de commandes
        ▲                                                         │
        │ événements blast://state (10/s), blast://playlist,      ▼
        │ blast://blocked, notice, cast, pads, obs, history       controller/ (thread dédié)
        └──────────────────────────────────────────────── │  possède Engine, Playlist, Caster,
                                                           │  AuraClient, Library
                                                           ├─ threads de décodage
                                                           ├─ threads de scan (dossiers, tags)
                                                           └─ threads de statistiques
```

| Fichier | Rôle |
|---|---|
| `src-tauri/src/main.rs` | Démarrage : journal, réglages, contrôleur, fichiers passés en argument |
| `src-tauri/src/api.rs` | Commandes Tauri appelées par l'interface (réglages, tests de connexion, export, mises à jour) |
| `src-tauri/src/controller/` | Thread qui possède le moteur, découpé par domaine (voir ci-dessous) |
| `src-tauri/src/playlist.rs` | Playlist (insertion, déplacement, retrait), parcours récursif des dossiers, lecture rapide des tags |
| `src-tauri/src/config.rs` | Réglages `config.toml` (jamais de mot de passe), écriture atomique |
| `src-tauri/src/secrets.rs` | Mots de passe dans le trousseau du système, repli en mémoire |
| `src-tauri/src/journal.rs` | Journal `blast.log` (rotation à 1 Mo) |
| `src-tauri/src/update.rs` | Vérification des versions publiées sur GitHub |
| `src-tauri/src/themes.rs` | Thèmes intégrés (`themes/*.toml` compilés dans le binaire) et thèmes utilisateur |
| `ui/js/` | Modules ES : `core` (outils communs), `main` (démarrage et abonnements), `decks`, `playlist`, `vlist`, `dnd`, `cast`, `settings`, `voice`, `pads`, `obs`, `shortcuts`, `theme`, `update` |

**Contrôleur** (`controller/`) : `mod.rs` tient la boucle principale (toutes les 15 ms) et l'aiguillage ;
chaque domaine a son module et son état : `decks.rs` (platines, fondus, auto-mix), `playlist_ops.rs`,
`automation.rs` (règles de séparation, historique, purge), `cast.rs` (sorties, auditeurs),
`obs.rs`, `pads.rs`, `voice.rs`, et `views.rs` pour les structures envoyées à l'interface.
Le contrôleur n'appelle pas Tauri directement : il passe par le trait `Ui` ([CTL-01]), remplacé dans
`controller/tests.rs` par une interface qui enregistre les événements. Les tests y pilotent le contrôleur
de bout en bout (fichiers WAV générés, base SQLite en mémoire, moteur absent).

**Listes longues** : la playlist et l'historique passent par `vlist.js` ([UI-102]). Seules les lignes visibles
(plus une marge, [UI-101]) existent dans le DOM ; un espaceur donne la hauteur totale. Une playlist de 40 000 titres
se mélange donc sans reconstruire 40 000 lignes. Les motifs d'attente des règles de rotation arrivent à part
(`blast://blocked`, [CTL-154]) toutes les 30 s, sans renvoyer toute la playlist.

**Reprise de la playlist** : chaque publication de la playlist la marque comme modifiée ; au plus 2 s après
([CTL-155]), un instantané part vers un fil dédié qui écrit `playlist.json` (fichier temporaire, `sync_all`, renommage,
[PL-18]). Les tags y sont gardés, la reprise ne relit donc aucun fichier audio. À la fermeture, `RunEvent::Exit`
([APP-44]) fait écrire la playlist tout de suite.

**Journal** : `blast.log` dans le dossier de données (`~/.local/share/fr.black-lab.blast/logs` sous Linux,
`%LOCALAPPDATA%\fr.black-lab.blast\logs` sous Windows). Avertissements et erreurs affichés à l'utilisateur
y sont recopiés, ainsi que les erreurs qui n'ont pas besoin de l'interrompre (statistiques, bibliothèque).
Aucun mot de passe n'y est écrit.

**Mises à jour** : au démarrage (désactivable), `check_update` interroge
`api.github.com/repos/D-Goth/BLAST-Broadcaster/releases/latest` avec le magasin de certificats du système
(fonctionne derrière un proxy d'entreprise). Une version plus récente et non ignorée est annoncée par une
notification ; rien n'est téléchargé. Le numéro de version vient du workspace (`Cargo.toml`), seule source.

**Nomenclature** : voir [NOMENCLATURE.md](NOMENCLATURE.md). Les définitions sont dans `nomenclature.toml`,
le document est généré et vérifié par `scripts/nomenclature.py`.

**Commandes de l'interface** (`UiCommand`, JSON `{"type": …}`) : `refresh`, `add_paths`, `load_paths`, `remove`, `move`,
`clear`, `load_entry`, `play`, `pause`, `stop`, `eject`, `seek`, `crossfader`, `crossfade`, `auto_mix`, `fade_seconds`,
`cast_start`, `cast_stop`, `next`, `mic_open`, `pad_play`, `pad_stop_all`, `jingle_play`, `jingle_stop_all`, `jingle_rescan`,
`obs`, `history_clear` ([CTL-26]).

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

## Serveur Icecast (module de gestion)

Gérer son propre serveur Icecast depuis BLAST, sans éditer `icecast.xml` à la main.

```
 onglet Serveur Icecast (ui/js/icecast.js) ── invoke("ice_*") ──▶ icecast.rs (IceManager, profils, trousseau)
                                                             │
                                  blast-ice ◀────────────────┘
                                  ├─ config.rs  : icecast.xml ⇄ IceConfig, validation, sauvegardes
                                  ├─ client.rs  : /admin/stats, listclients, killclient, killsource, moveclients, metadata
                                  ├─ stats.rs   : lecture des réponses XML
                                  └─ process.rs : détection, lancement, arrêt, PID, fin des journaux
```

| Fichier | Rôle |
|---|---|
| `blast-ice/src/config.rs` | Le fichier est lu comme un arbre XML complet (`xmltree`). Seules les balises gérées sont réécrites : commentaires, relais, authentification des mountpoints, en-têtes HTTP et sécurité restent intacts. Une valeur vide retire la balise. Avant chaque écriture : validation, copie `icecast.xml.bak-<horodatage>` (10 gardées), fichier temporaire puis renommage |
| `blast-ice/src/client.rs` | Interface admin d'Icecast en HTTP(S) avec authentification Basic, délai de 5 s, certificats du système |
| `blast-ice/src/process.rs` | Lancement `icecast -c icecast.xml` depuis le dossier d'installation (les chemins relatifs `./web`, `./log` en partent), sans fenêtre de console sous Windows ; le PID est noté pour reprendre le contrôle après un redémarrage de BLAST |
| `app/src-tauri/src/icecast.rs` | Profils de serveurs (`[[icecast.servers]]` dans `config.toml`), mot de passe admin dans le trousseau (`icecast:<id>`), commandes Tauri |

**Local ou distant** : un serveur local est piloté entièrement (démarrage, configuration, journaux). Un serveur distant
(VPS) est surveillé et administré par son interface admin ; sa configuration et son démarrage passeront par SSH.

**Windows et Program Files** : l'installateur d'Icecast place `icecast.xml` dans un dossier protégé. BLAST le détecte
(fichier en lecture seule) et propose de **copier la configuration dans BLAST** : la copie va dans le dossier de réglages
(`%APPDATA%\fr.black-lab.blast\icecast\<id>\icecast.xml`), avec des chemins absolus vers `web` et `admin` de
l'installation et des journaux dans le dossier de données de BLAST. L'original n'est jamais modifié.

**Mots de passe** : Icecast lit les siens en clair dans `icecast.xml` ; un hash ne lui servirait à rien, et BLAST doit
envoyer le vrai mot de passe admin à chaque requête. Le mot de passe admin est donc rangé chiffré dans le trousseau du
système, jamais dans `config.toml`. Pour un serveur local sans mot de passe enregistré, il est relu dans `icecast.xml`.
Une connexion admin en HTTP vers un hôte distant est signalée : le mot de passe y circule en clair.

**Tableau de bord** : `ice_status` toutes les 2 s tant que l'onglet est affiché, rien quand les Réglages sont fermés. Le débit sortant est calculé sur 30 s à partir des octets envoyés. Les mountpoints déclarés sans
source apparaissent « en attente », avec leur secours.

Testé contre Icecast 2.4.4 : lecture et réécriture de la configuration, lancement et arrêt, statistiques admin,
liste et déconnexion d'auditeurs, changement de titre, déplacement d'auditeurs (refus lisible si la destination n'existe pas).

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

Le mixeur produit deux mélanges : le mélange **antenne** (platines, pads et micro), envoyé aux encodeurs,
et le mélange **local** pour les enceintes et le casque, identique mais sans le micro, sauf si le retour est activé.

Le retard du micro est borné : si le tampon dépasse 80 ms (horloges d'entrée et de sortie différentes),
l'excédent est abandonné. Le micro s'ouvre et se ferme avec une rampe de 20 ms pour éviter les clics.
Les sons des pads sont décodés en arrière-plan et gardés en mémoire.

## Aura — pilotage d'OBS (M7)

`blast-aura/src/client.rs` ouvre une connexion obs-websocket v5 persistante dans son propre thread :
identification (avec mot de passe si OBS en demande), abonnement aux événements, puis boucle
commandes ↔ messages avec un délai de lecture de 50 ms. En cas de coupure, nouvelle tentative après
2 s, puis 4, 8… jusqu'à 30 s ; un mot de passe refusé arrête les tentatives.

| Requête OBS | Usage |
|---|---|
| `GetSceneList`, `SetCurrentProgramScene` | Grille de scènes (ordre de l'interface d'OBS) |
| `GetStreamStatus`, `ToggleStream` | Bouton STREAM, minuteur ▶ (reprend la durée réelle si le stream tournait déjà) |
| `GetRecordStatus`, `ToggleRecord`, `ToggleRecordPause` | Boutons REC et PAUSE REC, minuteur ⏺ |
| `SaveReplayBuffer`, `SaveSourceScreenshot` | REPLAY et SNAP (PNG de la scène en cours) |
| `GetInputList`, `GetInputMute`, `GetInputVolume`, `ToggleInputMute`, `SetInputVolume` | Sources audio de capture uniquement |
| `SetInputSettings` (`text`) | Titre en cours dans une source texte OBS |

Les événements `CurrentProgramSceneChanged`, `StreamStateChanged`, `RecordStateChanged`,
`InputMuteStateChanged`, `InputVolumeChanged` tiennent l'interface à jour quand on agit directement dans OBS.

## Grilles horaires (pilier 2)

La logique pure est dans `blast-library/src/rotation.rs` (testée sans interface ni audio) :
catégories à règles ([LIB-46]), grilles ([LIB-49]), planning 7 × 24 ([LIB-52]), poids ([LIB-53]) et tirage
pondéré ([LIB-54]). Le contrôleur (`controller/rotation.rs`) scanne les dossiers de la bibliothèque en
arrière-plan (seuls les fichiers dont la date de modification a changé sont relus, [CTL-140]), garde les
titres en mémoire, et, quand l'auto-mix est actif et qu'une grille couvre l'heure en cours, complète la
playlist jusqu'à 3 titres d'avance ([CTL-145]). Chaque titre tiré porte l'étiquette de sa catégorie.

Le tirage ([CTL-146]) exclut les titres déjà prévus ou sur une platine, applique les mêmes règles de
séparation que la playlist ([CTL-150]) et évite de prévoir deux fois de suite le même artiste. Si une
catégorie n'a plus aucun titre conforme, les règles sont assouplies pour elle (avertissement unique) ;
si elle est vide, la case est passée (avertissement unique).

## Verre dépoli

Purement dans l'interface, donc identique sous Linux et Windows : l'image choisie est copiée dans le dossier
de réglages (`background-<horodatage>.<ext>`) et servie par le protocole de fichiers de Tauri, dont la
portée est limitée à ces fichiers (`assetProtocol.scope` dans `tauri.conf.json`). En mode verre, les couleurs
de fond du thème deviennent translucides ([UI-87]) et les panneaux floutent ce qui est derrière eux avec
`backdrop-filter` ([STY-17]). La fenêtre reste opaque : voir le bureau à travers demanderait une fenêtre
transparente et le flou du compositeur (Mica / Acrylic sous Windows, KWin sous KDE), non retenu pour l'instant.

## Bibliothèque et historique (M4)

`blast-library` sépare trois couches :

| Couche | Rôle |
|---|---|
| `HistoryStore` ([LIB-25]) | Interface de stockage. `SqliteStore` ([LIB-08]) l'implémente (`blast.db`, mode WAL) ; un pilote MariaDB / MySQL pourra s'y brancher pour une base externe partagée entre plusieurs postes |
| `HistoryCache` ([LIB-30]) | Tampons en mémoire : un flux global et un tampon par catégorie (morceaux, jingles, pubs), 200 passages chacun par défaut (réglable) |
| `History` ([LIB-36]) | Point d'entrée unique : chaque passage validé à l'antenne est écrit en base et dans les tampons en même temps ; au démarrage, les tampons sont pré-remplis depuis la base |

| Table | Contenu |
|---|---|
| `tracks` | Chemin, titre, artiste, album, genre, année, durée, type, nombre de passages, dernier passage |
| `historique_diffusion` | Chaque passage : type (`morceau`, `jingle`, `pub`), titre, artiste, heure de début, durée réellement diffusée, diffusion en direct ou non, auditeurs ; index par date, par type et date, par artiste et par fichier |

La base garde tout l'historique (purge réglable, désactivable). Les tampons servent l'onglet Historique
et ses filtres sans aucune requête. Les règles de séparation ([LIB-39]) s'appliquent aux morceaux
seulement : elles sont vérifiées en mémoire tant que le tampon des morceaux couvre la fenêtre des règles,
et ne passent par la base que pour des règles plus longues que ce qu'il contient (plusieurs jours).
L'export CSV lit la base, filtré par catégorie et par période.

**Types** : un fichier est jingle ou pub si l'un de ses dossiers porte un mot-clé réglable
(« JINGLE », « PUB », « PUBLICITE » par défaut ; singulier ou pluriel, accents ignorés) ([PL-09]).
Jingles et pubs ne changent pas le titre envoyé aux serveurs ni à OBS.

**Box à jingles** : les fichiers du dossier choisi sont décodés à l'avance et joués à la demande
par-dessus la musique, sur des voix « cart » du mixeur (numéros à partir de 1000, volume propre,
arrêt séparé des pads) ([MIX-16], [CTL-131]). Chaque lancement est un passage de type jingle.
Les sons de la Sound FX (pads) restent hors historique.

## Traitement audio (M6)

Tout est écrit dans `blast-core/src/dsp.rs`, sans dépendance externe et sans allocation dans le thread audio.

```
 voix : coupe-bas → porte de bruit → filtres passe-haut/passe-bas → grave / présence / aigu
        → saturation (tanh, ou asymétrique façon lampe) → compresseur → de-esser → souffle et craquements
        → égaliseur de l'utilisateur → gain
 antenne : (platines + pads + micro) → AGC (vise un niveau RMS, ±9 à 12 dB, gelée sous -50 dB)
           → compresseur (genou doux) → limiteur à anticipation de 3 ms (plafond -0,5 à -1 dBFS)
```

Le gain de la chaîne antenne est calculé sur le mélange antenne et appliqué aussi au mélange local
(enceintes, casque) : les deux restent alignés, et l'animateur entend le même son que les auditeurs, micro en moins.

| Préréglage antenne | AGC | Compresseur | Plafond |
|---|---|---|---|
| Aucun traitement | — | — | -0,3 dBFS |
| Doux | — | 2:1 à -16 dB | -1 dBFS |
| Radio FM | cible -17 dB | 3,5:1 à -20 dB | -0,8 dBFS |
| Puissant | cible -14 dB | 5:1 à -22 dB | -0,5 dBFS |

## Dépendances envisagées

| Besoin | Crate / bibliothèque | Licence |
|---|---|---|
| Sortie audio multi-OS | `cpal` | Apache-2.0 |
| Décodage MP3/AAC/FLAC/Vorbis/WAV | `symphonia` | MPL-2.0 |
| Encodage Opus / Vorbis / FLAC | libopus / libvorbis / libFLAC | BSD |
| Rééchantillonnage | `rubato` | MIT |
| Interface | Tauri 2, tauri-plugin-dialog | MIT / Apache-2.0 |
| Thèmes | `toml`, `serde_json` | MIT / Apache-2.0 |
| Base de données | `rusqlite` (SQLite intégré) | MIT, SQLite domaine public |
| Dates de l'export | `chrono` | MIT / Apache-2.0 |
| WebSocket (OBS, chat) | `tokio-tungstenite` | MIT |
| Files temps réel | `crossbeam-channel`, `rtrb` | MIT / Apache-2.0 |
| Encodage MP3 | `mp3lame-encoder` (LAME compilé avec le projet) | LGPL (LAME), MIT |
| Trousseau | `keyring` | MIT / Apache-2.0 |
| icecast.xml | `xmltree`, `xml-rs` | MIT |
| Mots de passe aléatoires | `getrandom` | MIT / Apache-2.0 |
| OBS | `tungstenite`, `sha2`, `base64` | MIT / Apache-2.0 |
| Ligne de commande, WAV | `clap`, `hound` | MIT / Apache-2.0 |

## Règle de licence des dépendances

Le code étant sous PolyForm Noncommercial, **aucune dépendance GPL ne doit être liée au binaire**
(la GPL interdit d'ajouter une restriction non commerciale). LGPL en liaison dynamique, MIT, Apache, BSD et MPL sont acceptées.
FFmpeg n'est envisageable qu'en compilation LGPL, sans `--enable-gpl`.
LAME est compilé dans le binaire : c'est compatible avec la LGPL tant que le code source complet de BLAST
reste disponible, ce qui permet à chacun de recompiler avec une autre version de LAME.
L'encodage AAC (fdk-aac) a une licence de redistribution particulière : à traiter au jalon M9.
La vérification des mises à jour utilise ureq, rustls, ring et rustls-platform-verifier (MIT, Apache 2.0, ISC) ;
l'ouverture des liens et dossiers utilise open (MIT).

## Thèmes

Les thèmes sont des fichiers TOML dans `themes/`. L'interface n'utilise jamais de couleur en dur,
uniquement les tokens définis dans le thème, exposés en variables CSS (`--bg`, `--accent`, `--deck-a`…).
Les couleurs de mesure (`meter_*`) et d'alerte de saturation (`clip`) sont séparées de l'accent
pour rester lisibles quel que soit le thème.
