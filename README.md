# BLAST Broadcaster

**Black-Lab Audio Streaming Tool** — console de webradio et de live multi-OS (Linux et Windows ; macOS prévu).

BLAST Broadcaster réunit dans un seul logiciel ce qu'il fallait jusqu'ici assembler à la main :
la console d'antenne d'une webradio (à la manière de SAM Broadcaster), la diffusion vers les serveurs
de streaming audio, et le pilotage d'un live vidéo avec webcams et chat (talk show, live DJ, émission).

![BLAST Broadcaster — thème Black-Lab](docs/img/interface-black-lab.png)

> Statut : **les trois piliers sont en place** (automatisation et historique, grilles horaires, son antenne), avec
> la diffusion, le serveur Icecast intégré, le pilotage d'OBS et le chat multi-plateformes. Voir la
> [feuille de route](ROADMAP.md) pour la suite.

## Fonctionnalités

✅ disponible · ⏳ prévu

**Antenne**
- ✅ Deux platines, crossfade automatique ou manuel, auto-mix, bouton Suivant
- ✅ Playlist de plusieurs dizaines de milliers de titres, mélange (avec espacement des artistes), retrait des doublons, enregistrement et ouverture en M3U, reprise automatique après un plantage
- ✅ Grilles horaires (clockwheels) : catégories, grilles, émissions M3U, planning de la semaine, tirage pondéré
- ✅ Règles de séparation artiste / titre, historique typé (morceaux, jingles, pubs) exportable
- ✅ Box à jingles, Sound FX (36 pads, F1 à F12)
- ✅ Micro avec ducking automatique (niveau, descente et remontée réglables), égaliseur, effets de voix
- ✅ Son antenne : AGC, compresseur, limiteur à anticipation (presets Doux, Radio FM, Puissant)
- ⏳ Jusqu'à 4 platines, cue points, pré-écoute casque (PFL), contrôleur MIDI, voice tracking

**Diffusion**
- ✅ Plusieurs sorties simultanées en MP3, chacune avec son serveur et son débit
- ✅ Icecast, Shoutcast v1 et v2, titre en cours, reconnexion automatique, auditeurs et pic en direct
- ✅ Gestion de son propre serveur Icecast : lancement, configuration sans éditer le XML, surveillance
- ⏳ Formats AAC, Opus, Ogg Vorbis ; configuration d'un Icecast sur VPS par SSH

**Live**
- ✅ Pilotage d'OBS Studio via obs-websocket (module Aura), titre en cours vers une source texte OBS
- ✅ Chat Twitch direct, et YouTube, TikTok, Kick, Facebook… par Social Stream Ninja
- ✅ Commandes viewers : `!np`, `!request` (demandes à valider), `!sfx`, annonce des titres

**Interface**
- ✅ Thèmes personnalisables et partageables (Light, Dark et Black-Lab fournis), couleur par platine, mode rainbow
- ✅ Verre dépoli sur une image de fond, Linux et Windows

## Modules

| Crate | Rôle |
|---|---|
| `blast-core` | Moteur audio : platines, mixage, crossfade, traitement |
| `blast-cast` | Encodeurs et envoi vers les serveurs de streaming |
| `blast-library` | Bibliothèque musicale, file d'attente, automatisation |
| `blast-aura` | Pilotage d'OBS Studio |
| `blast-live` | Chat : Twitch direct (IRC chiffré, lecture anonyme) et Social Stream Ninja (WebSocket), commandes viewers |
| `blast-ice` | Gestion d'un serveur Icecast : icecast.xml, statistiques, administration, lancement |
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
Les captures OBS (SNAP) vont dans `~/Images/BLAST`.
PipeWire et PulseAudio sont pris en charge via leur couche de compatibilité ALSA.

## Installer sous Windows 10 / 11

Tout se fait dans l'**Invite de commandes** (`cmd`), pas besoin de PowerShell.

**1. Les outils (une seule fois)**

```bat
winget install --id Git.Git -e
winget install --id Rustlang.Rustup -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Sans winget : [Git pour Windows](https://git-scm.com/download/win), [rustup](https://rustup.rs) (il propose d'installer
les outils C++ de Visual Studio : accepte). WebView2, utilisé pour l'interface, est déjà présent sur Windows 10 et 11.
**Ferme puis rouvre l'invite de commandes** après ces installations.

**2. Récupérer et installer BLAST**

```bat
git clone https://github.com/D-Goth/BLAST-Broadcaster.git
cd BLAST-Broadcaster
scripts\install-windows.bat
```

Le script compile, copie BLAST dans `%LOCALAPPDATA%\Programs\BLAST Broadcaster` et crée l'entrée
**BLAST Broadcaster** dans le menu Démarrer (clic droit → Épingler à la barre des tâches).

- Mettre à jour : `git pull` puis `scripts\install-windows.bat`
- Désinstaller : `scripts\install-windows.bat /uninstall` (les réglages restent dans `%APPDATA%\fr.black-lab.blast`)

Sous Windows, les mots de passe sont rangés dans le Gestionnaire d'identifiants, et le son passe par WASAPI.
Les captures OBS (SNAP) vont dans `Images\BLAST`.

## Utiliser l'interface

| Action | Comment |
|---|---|
| Ajouter des titres | **+ Fichiers** (sélection multiple, playlists M3U comprises), **+ Dossier** (sous-dossiers compris), ou glisser depuis Dolphin ou l'Explorateur Windows sur la playlist |
| Charger une platine | Glisser un titre (ou un fichier depuis Dolphin) sur la platine, boutons **A** / **B** de la ligne, ou double-clic (platine libre) |
| Réordonner | Glisser un titre dans la playlist |
| Enchaîner | **FONDU → A/B** démarre l'autre platine et fond vers elle, puis arrête la précédente |
| Auto-mix | Pioche dans la playlist et enchaîne tout seul, avec la durée de fondu choisie (zone hachurée sur la barre de progression) |
| Titre suivant | Bouton **⏭** d'une platine : fondu vers l'autre platine ; si elle est vide, le prochain titre de la playlist y est chargé puis enchaîné |
| Se déplacer | Cliquer sur la barre de progression |
| Micro | **Appuyer pour parler** (ou **Espace** maintenu) : ouvert tant qu'on appuie ; **🔒 VERROU** (ou touche **M**) : verrouille le micro ouvert, re-cliquer pour le refermer |
| Retour micro | **Réglages → Audio** : le micro part toujours vers la diffusion ; « Entendre mon micro » le renvoie aussi vers les enceintes/le casque (désactivé par défaut, pas d'écho ni de Larsen) |
| Ducking | **AUTO** : la musique baisse automatiquement (niveau réglable, -12 dB par défaut) quand le micro est ouvert ; **Temps du ducking** règle la descente (0,2 s par défaut) et la remontée (0,8 s) de la musique |
| Égaliseur micro | Grave, médium, aigu (±12 dB) et coupe-bas ; volume micro et volume musique séparés |
| Effets voix | Sélecteur dans l'en-tête du Voice FX : Naturel, Podcast chaleureux, Voix radio FM, ASMR, Radio TSF 1930, Téléphone, Mégaphone (porte de bruit, filtres, saturation « lampe », compresseur, de-esser, souffle et craquements selon l'effet) |
| Son antenne | Sélecteur **Son** du panneau MIX : Aucun traitement, Doux, Radio FM, Puissant (AGC, compresseur, limiteur à anticipation). Le VU **RÉD** montre la réduction de gain appliquée |
| Mélanger | **Mélanger ▾** dans la barre de la playlist : ordre aléatoire, mélange qui espace au maximum les titres d'un même artiste, ou retrait des doublons (même fichier, ou même titre et même artiste) |
| Playlists M3U | **Enregistrer…** écrit la playlist en M3U (`.m3u8`, lisible par SAM, Winamp, VLC). Pour rouvrir une playlist : **+ Fichiers**, glisser le fichier `.m3u` sur la playlist, ou le passer en argument au lancement |
| Reprise | La playlist est enregistrée deux secondes après chaque changement (`playlist.json` à côté de `blast.db`) et rechargée au démarrage, avec en tête les titres qui étaient sur les platines : après un plantage ou une coupure de courant, tout est là |
| Historique | Onglet **HISTORIQUE** de la playlist : chaque passage avec l'heure, le type (morceau, jingle, pub), la durée réellement diffusée, l'état de la diffusion et le nombre d'auditeurs ; filtres **Tout / Morceaux / Jingles / Pubs** ; **Exporter…** par catégorie et par période (mois en cours par défaut), pour les bilans et les déclarations de droits |
| Grilles (clockwheels) | **Réglages → Grilles** : ajoute les dossiers de ta musique (**Bibliothèque**), crée des **Catégories** (genres, années, BPM, dossiers, type), compose des **Grilles** (suite de catégories jouée en boucle), crée des **Émissions** (une playlist M3U jouée dans l'ordre, pour un événement ou une émission spéciale) et peins le **Planning** de la semaine. Une catégorie peut aussi piocher dans une ou plusieurs playlists M3U (thème, style). Avec l'auto-mix actif, BLAST garde 3 titres d'avance dans la playlist selon la grille de l'heure (affichée à côté de MIX), en favorisant les titres pas entendus depuis longtemps et en respectant les règles de séparation ; tu peux tout corriger avant que ça passe |
| Verre dépoli | **Réglages → Apparence** : choisis une image de fond, les panneaux deviennent translucides et floutés par-dessus ; flou, opacité et assombrissement réglables en direct, désactivable à tout moment |
| Jingles | Onglet **JINGLES** à côté de la Sound FX : **Dossier…** choisit le dossier de tes jingles, clic sur un jingle pour le lancer à l'antenne par-dessus la musique (re-clic = stop), volume et **STOP** propres. Les fichiers rangés dans un dossier nommé « Jingles » ou « Pubs » (mots-clés réglables dans Réglages → Automation) sont aussi reconnus dans la playlist |
| Règles de séparation | **Réglages → Automation** : pas le même artiste avant 120 min, pas le même titre avant 240 min (réglables). L'auto-mix et le bouton Suivant sautent les titres en attente, marqués d'une horloge dans la playlist |
| Soundboard | 36 pads sur 3 pages ; clic ou **F1 à F12** pour jouer (re-clic = stop), **STOP** coupe tout ; glisser un fichier (ou un titre de la playlist) sur un pad pour l'assigner ; clic droit : choisir, renommer, vider |
| Thèmes | Sélecteur en haut à droite ; **Thèmes…** indique le dossier où déposer tes propres fichiers `.toml` |
| Diffuser | **Réglages → Diffusion** : ajouter une sortie (Icecast, Shoutcast v1 ou v2), **Tester la connexion**, **Enregistrer** ; puis bouton **DIFFUSER** dans la barre du haut pour lancer ou arrêter la diffusion (durée et auditeurs s'y affichent) ; le **+** à côté ouvre le détail de chaque sortie |
| Chat | **Réglages → Chat** : active une ou plusieurs sources, puis onglet **CHAT** à côté de la playlist → **Connecter**. **Twitch direct** : indique ta chaîne (lecture anonyme) ; pour que BLAST réponde, ajoute un compte et son jeton (`twitch token -u -s "chat:read chat:edit"` avec Twitch CLI). **Social Stream Ninja** (gratuit) : YouTube, TikTok, Kick, Facebook et des dizaines d'autres ; active dans SSN « Enable remote API control of extension » et « Send chat messages to API server », puis colle l'identifiant de session. Chaque message porte le logo de sa plateforme, les réponses repartent sur la plateforme du viewer |
| Commandes viewers | `!np` répond avec le titre à l'antenne ; `!request Artiste - Titre` (ou `!sr`) cherche le titre dans la bibliothèque et l'ajoute aux **Demandes** à droite du chat : **Jouer ensuite** le met en tête de playlist, **Refuser** le retire ; `!sfx nom` lance le pad Sound FX du même nom (désactivé par défaut). Option : annoncer chaque nouveau titre dans le chat |
| Serveur Icecast | **Réglages → Serveur Icecast** : ajoute ton Icecast local (détecté dans Program Files) ou un VPS. **Démarrer / Arrêter**, tableau de bord en direct (auditeurs, pic, sources, débit, graphique, mountpoints), **Configuration** et **Mountpoints** par formulaires (une sauvegarde de `icecast.xml` avant chaque écriture), **Auditeurs** avec déconnexion, **Journal**. **Diffuser BLAST ici** crée la sortie correspondante dans Réglages → Diffusion |
| OBS | **Réglages → Aura / OBS** : adresse, port, mot de passe et source texte « Now Playing » ; puis onglet **AURA · OBS** (à côté de SOUND FX) ou bouton **OBS** de la barre du haut |
| Pilotage OBS | STREAM, REC, PAUSE REC, REPLAY, SNAP (capture dans `~/Images/BLAST`), MUTE et volume d'une source audio, changement de scène ; minuteurs ▶ stream et ⏺ enregistrement dans la barre du haut |

**Diffusion** : le bouton passe au vert (**EN DIRECT n/n**) quand toutes les sorties actives sont connectées.
Il affiche aussi la **durée de diffusion** et le **nombre d'auditeurs** (👤 en cours · max), relevés toutes les 10 secondes
sur le serveur (`status-json.xsl` pour Icecast, `7.html` pour Shoutcast v1, `stats` pour Shoutcast v2). Le détail par sortie
est dans le panneau ouvert par le **+** à côté du bouton, dans son infobulle et dans Réglages → Diffusion. En cas de coupure,
BLAST se reconnecte tout seul toutes les 5 secondes ; un mot de passe refusé arrête la sortie concernée.
Le titre en cours (« Artiste - Titre » de la platine à l'antenne) est envoyé au serveur à chaque changement.

**Mots de passe** : rangés dans le trousseau du système (KWallet sous KDE), jamais dans le fichier de réglages.
Si le trousseau est indisponible, BLAST le signale et garde le mot de passe en mémoire pour la session uniquement.
Les autres réglages sont dans `~/.config/fr.black-lab.blast/config.toml` (Windows : `%APPDATA%\fr.black-lab.blast\config.toml`).

Le micro se choisit dans **Réglages → Audio** (pris en compte au redémarrage). Le VU-mètre MIC montre le niveau
d'entrée même micro fermé, pour régler son gain avant de prendre l'antenne.

Un titre chargé mais pas encore joué, remplacé par un autre, retourne en tête de playlist.
Les fichiers illisibles restent dans la playlist, marqués ⚠, et sont sautés par l'auto-mix.

## Premiers pas avec Icecast

Icecast est un serveur de diffusion libre et gratuit (Windows et Linux), qui remplace le serveur SHOUTcast.
La différence qui surprend en venant de SHOUTcast : le **point de montage** (mountpoint).

- SHOUTcast v1 diffuse un seul flux par port : les auditeurs ouvrent `http://adresse:8000/`.
- Icecast peut héberger plusieurs flux sur le même port, chacun à son adresse : `http://adresse:8000/live`
  pour la radio en 320 kb/s, `http://adresse:8000/mobile` en 64 kb/s… `/live` n'est qu'un nom, choisis le tien.
- **Inutile de déclarer un point de montage** : Icecast accepte n'importe lequel du moment que la source donne le
  mot de passe des sources. Il apparaît quand BLAST se connecte et disparaît quand il se déconnecte.

Le chemin le plus court :

1. **Réglages → Serveur Icecast** : ajoute ton installation (détectée dans Program Files sous Windows).
2. Onglet **CONFIGURATION** : si le fichier est en lecture seule (Program Files), **Copier la configuration dans
   BLAST** ; change les trois mots de passe (le bouton dé en génère), puis **Enregistrer et démarrer**. Laisse vide
   le champ « Compatibilité Shoutcast » des ports, réservé aux vieux logiciels.
3. **TABLEAU DE BORD** → **Créer la sortie BLAST vers /live** (ou, à la main, Réglages → Diffusion : Icecast 2,
   `localhost`, port 8000, point de montage `/live`, identifiant `source`, mot de passe des sources).
4. **DIFFUSER**, puis écouter dans VLC : `http://adresse-du-pc:8000/live`.

L'onglet **MOUNTPOINTS** est facultatif : il sert à donner des règles à un flux précis (limite d'auditeurs, mot de
passe propre, **secours** vers un autre flux si ta diffusion tombe, nom et genre imposés).

Pour être écouté depuis Internet, il faut aussi ouvrir le port d'Icecast (8000 par défaut) dans le pare-feu de
l'ordinateur et le rediriger sur ta box.

## Mettre à jour

BLAST vérifie au démarrage si une nouvelle version est publiée sur GitHub et te prévient
(désactivable dans **Réglages → À propos**, où se trouve aussi le bouton « Vérifier maintenant »).
Rien n'est téléchargé automatiquement. Pour mettre à jour, dans le dossier du projet :

- Linux : `git pull && ./scripts/install-linux.sh`
- Windows (invite de commandes) : `git pull` puis `scripts\install-windows.bat`

En cas de souci, l'onglet **À propos** ouvre le dossier du journal `blast.log` (aucun mot de passe n'y figure).

## Développer

```bash
cargo build --release
cargo test
./target/release/blast-broadcaster ~/Musique
```

Le code ne contient pas de commentaires : chaque élément porte un code de référence (`// [MIX-12]`)
défini dans [docs/NOMENCLATURE.md](docs/NOMENCLATURE.md). Après une modification, régénérer ce document :

```bash
python3 scripts/nomenclature.py          # régénère docs/NOMENCLATURE.md
python3 scripts/nomenclature.py --check  # ce que vérifie la CI
```

La CI GitHub (Linux et Windows) vérifie le formatage, clippy sans avertissement, les tests et la nomenclature.
Elle utilise une version de Rust figée (`.github/workflows/ci.yml`) : on la monte volontairement, pour qu'une
nouvelle règle de clippy ne casse pas la CI sans prévenir.

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
