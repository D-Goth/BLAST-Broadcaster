# Nomenclature de BLAST Broadcaster

Le code ne contient pas de commentaires explicatifs : chaque élément important porte un code de référence
(`// [MIX-12]` en Rust et en JavaScript, `/* [STY-03] */` en CSS, `<!-- [HTM-02] -->` en HTML).
Ce document associe chaque code à sa définition et à un extrait du code d'origine.

Fichier généré par `python3 scripts/nomenclature.py` à partir de `docs/nomenclature.toml` :
ne pas le modifier à la main.

## Règles

- Un code par unité : type, fonction, méthode métier, commande, module, section de style ou zone de page.
- Les accesseurs simples et les implémentations de traits standards (Display, Error, From, Default, Drop)
  relèvent du code de leur type.
- Un nouveau code prend le numéro suivant de son préfixe ; un code n'est jamais réattribué.
  Un élément supprimé garde sa définition dans `nomenclature.toml` avec `retire = true`.
- Les tests ne sont pas codés.
- `python3 scripts/nomenclature.py --check` vérifie que chaque code est défini, que chaque définition
  est utilisée, qu'aucun commentaire libre n'est présent et que ce document est à jour.

## Préfixes

| Préfixe | Périmètre | Codes |
|---|---|---|
| [`CORE`](#core) | Types communs du moteur audio (blast-core) | 3 |
| [`DEC`](#dec) | Décodage, tags et rééchantillonnage (blast-core) | 13 |
| [`DSP`](#dsp) | Traitement du signal : filtres, dynamique, chaîne antenne, effets de voix (blast-core) | 20 |
| [`MIX`](#mix) | Mixeur temps réel (blast-core) | 17 |
| [`ENG`](#eng) | Moteur audio et périphériques (blast-core) | 19 |
| [`CAST`](#cast) | Sorties de diffusion, encodage MP3 et fils d'envoi (blast-cast) | 18 |
| [`PROT`](#prot) | Protocoles Icecast / Shoutcast et statistiques (blast-cast) | 23 |
| [`AURA`](#aura) | Pilotage d'OBS par obs-websocket (blast-aura) | 21 |
| [`LIB`](#lib) | Bibliothèque SQLite, historique et règles de séparation (blast-library) | 60 |
| [`LIVE`](#live) | Chat en direct (blast-live, en préparation) | 18 |
| [`ICE`](#ice) | Serveur Icecast : configuration icecast.xml, statistiques, administration et processus (blast-ice) | 45 |
| [`CLI`](#cli) | Outil en ligne de commande « blast » (blast-cli) | 14 |
| [`APP`](#app) | Démarrage de l'application et commandes Tauri | 45 |
| [`CFG`](#cfg) | Réglages enregistrés (config.toml) | 17 |
| [`SEC`](#sec) | Mots de passe dans le trousseau du système | 8 |
| [`ICM`](#icm) | Gestion des serveurs Icecast côté application | 30 |
| [`JRN`](#jrn) | Journal de diagnostic (blast.log) | 8 |
| [`UPD`](#upd) | Vérification des mises à jour sur GitHub | 10 |
| [`PL`](#pl) | Playlist côté application | 22 |
| [`THM`](#thm) | Thèmes de couleurs | 3 |
| [`CTL`](#ctl) | Contrôleur : fil central qui relie interface, moteur, diffusion, OBS et bibliothèque | 174 |
| [`UI`](#ui) | Interface : modules JavaScript | 137 |
| [`STY`](#sty) | Interface : feuille de style | 20 |
| [`HTM`](#htm) | Interface : structure de la page | 39 |

## CORE

Types communs du moteur audio (blast-core)

### [CORE-01] Identifiant de platine

Désigne la platine A ou B ; donne son index, la platine opposée et le côté du crossfader qui la met à l'antenne.

`crates/blast-core/src/lib.rs` ligne 15

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckId {
    A,
    B,
}
```

### [CORE-02] États d'une platine

Vide, chargée, en lecture, en pause ou terminée.

`crates/blast-core/src/lib.rs` ligne 64

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckState {
    Empty,
    Loaded,
    Playing,
    Paused,
    Finished,
}
```

### [CORE-03] Fondu à puissance constante

Calcule les gains des platines A et B selon la position du crossfader (loi cosinus / sinus) pour garder un volume perçu stable pendant le fondu.

`crates/blast-core/src/lib.rs` ligne 74

```rust
pub fn crossfade_gains(position: f32) -> (f32, f32) {
    let p = position.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (p.cos(), p.sin())
}
```


## DEC

Décodage, tags et rééchantillonnage (blast-core)

### [DEC-01] Titre décodé

Échantillons stéréo entrelacés en f32 à la fréquence du moteur, avec titre et artiste lus dans les tags.

`crates/blast-core/src/decode.rs` ligne 14

```rust
#[derive(Debug, Clone)]
pub struct Track {
    pub source: PathBuf,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub source_rate: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
}
```

### [DEC-02] Erreurs de décodage

Fichier illisible, format non pris en charge, absence de piste audio ou erreur du décodeur.

`crates/blast-core/src/decode.rs` ligne 47

```rust
#[derive(Debug)]
pub enum DecodeError {
    Io(std::io::Error),
    Format(String),
    NoAudioTrack,
    UnknownSampleRate,
    Resample(String),
}
```

### [DEC-03] Année des tags

Extrait une année sur quatre chiffres d'un champ de date libre.

`crates/blast-core/src/decode.rs` ligne 96

```rust
fn parse_year(v: &str) -> Option<i32> {
    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).take(4).collect();
    if digits.len() == 4 {
        digits.parse().ok()
    } else {
        None
    }
}
```

### [DEC-04] Lecture des tags

Récupère titre, artiste, album, genre et année dans les métadonnées lues par Symphonia.

`crates/blast-core/src/decode.rs` ligne 117

```rust
fn read_tags(rev: &MetadataRevision, t: &mut Tags) {
    for tag in rev.tags() {
        let v = tag.value.to_string();
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        match tag.std_key {
            Some(StandardTagKey::TrackTitle) if t.title.is_none() => t.title = Some(v.to_string()),
            Some(StandardTagKey::Artist) if t.artist.is_none() => t.artist = Some(v.to_string()),
            Some(StandardTagKey::AlbumArtist) if t.artist.is_none() => {
                t.artist = Some(v.to_string())
…
```

### [DEC-05] Extensions audio reconnues

Extensions acceptées dans la playlist, sur les platines et sur les pads.

`crates/blast-core/src/decode.rs` ligne 142

```rust
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "mp2", "m4a", "mp4", "aac", "alac", "flac", "ogg", "oga", "wav", "aif", "aiff", "aifc",
    "caf", "mka", "mkv", "webm",
];
```

### [DEC-06] Filtre des fichiers audio

Indique si un chemin porte une extension audio reconnue.

`crates/blast-core/src/decode.rs` ligne 148

```rust
pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}
```

### [DEC-07] Fiche d'un titre

Métadonnées et durée lues sans décoder l'audio, pour la playlist et la bibliothèque.

`crates/blast-core/src/decode.rs` ligne 156

```rust
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub bpm: Option<f32>,
    pub duration_secs: Option<f64>,
    pub sample_rate: Option<u32>,
}
```

### [DEC-08] Ouverture d'un fichier audio

Sonde le conteneur et retient la première piste audio décodable.

`crates/blast-core/src/decode.rs` ligne 181

```rust
fn open(path: &Path) -> Result<symphonia::core::probe::ProbeResult, DecodeError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    Ok(symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
…
```

### [DEC-09] Rassemblement des tags

Fusionne les tags du conteneur et ceux du flux, ces derniers étant prioritaires.

`crates/blast-core/src/decode.rs` ligne 197

```rust
fn collect_tags(probed: &mut symphonia::core::probe::ProbeResult) -> Tags {
    let mut t = Tags::default();
    if let Some(meta) = probed.metadata.get() {
        if let Some(rev) = meta.current() {
            read_tags(rev, &mut t);
        }
    }
    if let Some(rev) = probed.format.metadata().current() {
        read_tags(rev, &mut t);
    }
    t
}
```

### [DEC-10] Sonde rapide

Lit les tags et la durée d'un fichier sans le décoder entièrement.

`crates/blast-core/src/decode.rs` ligne 211

```rust
pub fn probe_file(path: &Path) -> Result<TrackInfo, DecodeError> {
    let mut probed = open(path)?;
    let tags = collect_tags(&mut probed);
    let track = probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoAudioTrack)?;
    let params = &track.codec_params;
    symphonia::default::get_codecs().make(params, &DecoderOptions::default())?;
    let duration_secs = match (params.n_frames, params.sample_rate) {
…
```

### [DEC-11] Décodage complet

Décode un fichier entier en stéréo f32 puis le rééchantillonne à la fréquence du moteur.

`crates/blast-core/src/decode.rs` ligne 240

```rust
pub fn decode_file(path: &Path, target_rate: u32) -> Result<Track, DecodeError> {
    let mut probed = open(path)?;
    let Tags { title, artist, .. } = collect_tags(&mut probed);
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoAudioTrack)?;
    let track_id = track.id;
    let source_rate = track
…
```

### [DEC-12] Rééchantillonnage

Convertit un signal stéréo entrelacé vers une autre fréquence avec un filtre sinc (rubato), par blocs.

`crates/blast-core/src/resample.rs` ligne 7

```rust
pub fn resample_stereo(interleaved: &[f32], from: u32, to: u32) -> Result<Vec<f32>, String> {
    if from == to || interleaved.is_empty() {
        return Ok(interleaved.to_vec());
    }
    let frames = interleaved.len() / 2;
    let ratio = to as f64 / from as f64;
    let expected = (frames as f64 * ratio).round() as usize;

    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    for f in 0..frames {
        left.push(interleaved[2 * f]);
…
```

### [DEC-13] BPM des tags

Lit un tempo entre 20 et 400 BPM dans le champ BPM des tags.

`crates/blast-core/src/decode.rs` ligne 106

```rust
fn parse_bpm(v: &str) -> Option<f32> {
    let n: f32 = v
        .split_whitespace()
        .next()?
        .replace(',', ".")
        .parse()
        .ok()?;
    (20.0..=400.0).contains(&n).then_some(n)
}
```


## DSP

Traitement du signal : filtres, dynamique, chaîne antenne, effets de voix (blast-core)

### [DSP-01] Filtre biquad

Cellule IIR du second ordre (forme directe transposée II) utilisée par tous les égaliseurs et filtres.

`crates/blast-core/src/dsp.rs` ligne 3

```rust
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}
```

### [DSP-02] Formules des filtres

Coefficients RBJ : passe-haut, passe-bas, cloche, plateaux grave et aigu ; réaccord sans perte d'état et traitement échantillon par échantillon.

`crates/blast-core/src/dsp.rs` ligne 21

```rust
impl Biquad {
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }
…
```

### [DSP-03] Conversion dB vers gain

Convertit des décibels en facteur linéaire.

`crates/blast-core/src/dsp.rs` ligne 148

```rust
pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
```

### [DSP-04] Conversion gain vers dB

Convertit un facteur linéaire en décibels, avec un plancher à -120 dB.

`crates/blast-core/src/dsp.rs` ligne 153

```rust
pub fn gain_to_db(g: f32) -> f32 {
    20.0 * g.max(1e-9).log10()
}
```

### [DSP-05] Coefficient de lissage

Coefficient d'un filtre à un pôle pour un temps d'attaque ou de relâchement donné en millisecondes.

`crates/blast-core/src/dsp.rs` ligne 158

```rust
fn coef(ms: f32, rate: f32) -> f32 {
    if ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (ms * 0.001 * rate)).exp()
    }
}
```

### [DSP-06] Réglages du compresseur

Seuil, taux, largeur du genou, attaque, relâchement et gain de compensation.

`crates/blast-core/src/dsp.rs` ligne 167

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompParams {
    pub threshold_db: f32,
    pub ratio: f32,
    pub knee_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
}
```

### [DSP-07] Compresseur

Compresseur à genou doux sur crête lissée ; renvoie le gain à appliquer et la réduction en cours.

`crates/blast-core/src/dsp.rs` ligne 178

```rust
#[derive(Debug, Clone, Copy)]
pub struct Compressor {
    p: CompParams,
    att: f32,
    rel: f32,
    gr_db: f32,
}
```

### [DSP-08] Réglages de l'AGC

Niveau cible, gain maximal, vitesses de montée et de descente, seuil de silence.

`crates/blast-core/src/dsp.rs` ligne 233

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgcParams {
    pub target_db: f32,
    pub max_gain_db: f32,
    pub min_gain_db: f32,
    pub gate_db: f32,
}
```

### [DSP-09] Contrôle automatique de gain

Rapproche lentement le niveau moyen de la cible ; le gain est figé sous le seuil de silence.

`crates/blast-core/src/dsp.rs` ligne 242

```rust
#[derive(Debug, Clone, Copy)]
pub struct Agc {
    p: AgcParams,
    ms_coef: f32,
    ms: f32,
    gain_db: f32,
    up: f32,
    down: f32,
}
```

### [DSP-10] Anticipation du limiteur

Le limiteur voit le signal 3 ms à l'avance.

`crates/blast-core/src/dsp.rs` ligne 284

```rust
pub const LIMITER_LOOKAHEAD_MS: f32 = 3.0;
```

### [DSP-11] Limiteur à anticipation

Retarde le signal de 3 ms pour baisser le gain avant la crête : aucun échantillon ne dépasse le plafond.

`crates/blast-core/src/dsp.rs` ligne 287

```rust
pub struct Limiter {
    ceiling: f32,
    len: usize,
    air: Vec<[f32; 2]>,
    local: Vec<[f32; 2]>,
    targets: Vec<f32>,
    pos: usize,
    g: f32,
    att: f32,
    rel: f32,
}
```

### [DSP-12] Réglages du traitement antenne

AGC, compresseur, limiteur et activation de la chaîne.

`crates/blast-core/src/dsp.rs` ligne 365

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MasterParams {
    pub agc: Option<AgcParams>,
    pub comp: Option<CompParams>,
    pub ceiling_db: f32,
    pub release_ms: f32,
}
```

### [DSP-13] Liste des préréglages antenne

Identifiants et libellés : aucun, doux, radio FM, puissant.

`crates/blast-core/src/dsp.rs` ligne 374

```rust
pub const MASTER_PRESETS: &[(&str, &str)] = &[
    ("aucun", "Aucun traitement"),
    ("doux", "Doux"),
    ("radio_fm", "Radio FM"),
    ("puissant", "Puissant"),
];
```

### [DSP-14] Préréglages antenne

Construit les réglages de la chaîne antenne à partir d'un identifiant.

`crates/blast-core/src/dsp.rs` ligne 382

```rust
pub fn master_preset(id: &str) -> MasterParams {
    match id {
        "aucun" => MasterParams {
            agc: None,
            comp: None,
            ceiling_db: -0.3,
            release_ms: 50.0,
        },
        "radio_fm" => MasterParams {
            agc: Some(AgcParams {
                target_db: -17.0,
                max_gain_db: 9.0,
…
```

### [DSP-15] Chaîne de traitement antenne

AGC, compresseur puis limiteur sur le mélange stéréo, avec la réduction mesurée pour le vumètre RÉD.

`crates/blast-core/src/dsp.rs` ligne 443

```rust
pub struct MasterChain {
    rate: f32,
    params: MasterParams,
    agc: Agc,
    comp: Compressor,
    limiter: Limiter,
    max_reduction: f32,
}
```

### [DSP-16] Réglages des effets de voix

Porte de bruit, filtres, saturation, compresseur, de-esser, souffle et craquements.

`crates/blast-core/src/dsp.rs` ligne 522

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceParams {
    pub gate_db: Option<f32>,
    pub hp_hz: f32,
    pub lp_hz: f32,
    pub low: (f32, f32),
    pub presence: (f32, f32, f32),
    pub high: (f32, f32),
    pub drive: f32,
    pub tube: bool,
    pub comp: Option<CompParams>,
    pub deess_db: Option<f32>,
…
```

### [DSP-17] Liste des préréglages voix

Identifiants et libellés des effets de voix proposés dans Voice FX.

`crates/blast-core/src/dsp.rs` ligne 560

```rust
pub const VOICE_PRESETS: &[(&str, &str)] = &[
    ("aucun", "Aucun effet"),
    ("naturel", "Naturel"),
    ("podcast", "Podcast chaleureux"),
    ("radio_fm", "Voix radio FM"),
    ("asmr", "ASMR"),
    ("tsf_1930", "Radio TSF 1930"),
    ("telephone", "Téléphone"),
    ("megaphone", "Mégaphone"),
];
```

### [DSP-18] Raccourci de compresseur

Construit des réglages de compresseur pour les préréglages de voix.

`crates/blast-core/src/dsp.rs` ligne 572

```rust
fn comp(
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_db: f32,
) -> Option<CompParams> {
    Some(CompParams {
        threshold_db,
        ratio,
        knee_db: 6.0,
        attack_ms,
…
```

### [DSP-19] Préréglages voix

Naturel, podcast, radio FM, ASMR, TSF 1930, téléphone et mégaphone.

`crates/blast-core/src/dsp.rs` ligne 590

```rust
pub fn voice_preset(id: &str) -> VoiceParams {
    let d = VoiceParams::default();
    match id {
        "naturel" => VoiceParams {
            gate_db: Some(-55.0),
            comp: comp(-22.0, 2.0, 8.0, 150.0, 2.0),
            deess_db: Some(-30.0),
            ..d
        },
        "podcast" => VoiceParams {
            gate_db: Some(-52.0),
            hp_hz: 80.0,
…
```

### [DSP-20] Boîte d'effets de voix

Traite le micro : porte, filtres, saturation tanh ou lampe, compression, de-esser large bande, souffle et craquements simulés.

`crates/blast-core/src/dsp.rs` ligne 664

```rust
pub struct VoiceFx {
    rate: f32,
    p: VoiceParams,
    hp1: Biquad,
    hp2: Biquad,
    lp1: Biquad,
    lp2: Biquad,
    low: Biquad,
    presence: Biquad,
    high: Biquad,
    deess_band: Biquad,
    deess_env: f32,
…
```


## MIX

Mixeur temps réel (blast-core)

### [MIX-01] Polyphonie Sound FX

Nombre maximal de sons de pads joués en même temps (16).

`crates/blast-core/src/mixer.rs` ligne 6

```rust
pub const MAX_VOICES: usize = 16;
```

### [MIX-02] Retard micro maximal

Au-delà de 80 ms de son micro en attente, le surplus est jeté pour garder une latence faible.

`crates/blast-core/src/mixer.rs` ligne 10

```rust
const MIC_MAX_BACKLOG_MS: usize = 80;
```

### [MIX-03] Réglages du micro

Gain, égaliseur trois bandes et passe-haut.

`crates/blast-core/src/mixer.rs` ligne 13

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MicParams {
    pub gain_db: f32,
    pub low_db: f32,
    pub mid_db: f32,
    pub high_db: f32,
    pub highpass: bool,
}
```

### [MIX-04] Commandes du mixeur

Messages envoyés au fil audio : platines, crossfader, auto-mix, pads, micro, ducking et traitements.

`crates/blast-core/src/mixer.rs` ligne 35

```rust
#[derive(Debug, Clone)]
pub enum Command {
    Load(DeckId, Arc<Track>),
    Eject(DeckId),
    Play(DeckId),
    Pause(DeckId),
    Stop(DeckId),
    Seek(DeckId, usize),
    SetDeckGain(DeckId, f32),
    SetCrossfader(f32),
    StartCrossfade {
        to: DeckId,
…
```

### [MIX-05] État publié d'une platine

État, position et durée en trames, renvoyés à chaque tick.

`crates/blast-core/src/mixer.rs` ligne 78

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckStatus {
    pub state: DeckState,
    pub position: usize,
    pub frames: usize,
}
```

### [MIX-06] Événements du mixeur

Fin de titre, transition automatique, fin de fondu, fin d'un son de pad et tick de mesure.

`crates/blast-core/src/mixer.rs` ligne 86

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Ended(DeckId),
    AutoTransition {
        from: DeckId,
        to: DeckId,
    },
    CrossfadeDone {
        to: f32,
    },
    SfxEnded(u16),
    Tick {
…
```

### [MIX-07] Voix Sound FX

Son de pad en cours de lecture et sa position.

`crates/blast-core/src/mixer.rs` ligne 110

```rust
struct Voice {
    pad: u16,
    track: Arc<Track>,
    pos: usize,
}
```

### [MIX-08] Chaîne micro

File d'entrée, filtres, gain et effets du micro avant le mélange.

`crates/blast-core/src/mixer.rs` ligne 117

```rust
struct MicChain {
    hp: Biquad,
    low: Biquad,
    mid: Biquad,
    high: Biquad,
}
```

### [MIX-09] Emplacement de platine

Titre chargé, position, état et gain d'une platine dans le fil audio.

`crates/blast-core/src/mixer.rs` ligne 125

```rust
#[derive(Debug, Default)]
struct Slot {
    track: Option<Arc<Track>>,
    pos: usize,
    playing: bool,
    gain: f32,
}
```

### [MIX-10] Fondu en cours

Positions de départ et d'arrivée du crossfader et durée du fondu.

`crates/blast-core/src/mixer.rs` ligne 159

```rust
#[derive(Debug, Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    total: usize,
    done: usize,
    outgoing: Option<usize>,
}
```

### [MIX-11] Mixeur temps réel

Mélange les deux platines, les sons de pads et le micro, sans aucune allocation dans le fil audio.

`crates/blast-core/src/mixer.rs` ligne 169

```rust
pub struct Mixer {
    sample_rate: u32,
    decks: [Slot; 2],
    crossfader: f32,
    fade: Option<Fade>,
    master: f32,
    automix: bool,
    automix_frames: usize,
    tick_every: usize,
    tick_count: usize,
    peak: [f32; 2],
    voices: Vec<Voice>,
…
```

### [MIX-12] Application des commandes

Exécute une commande de l'interface ; les titres remplacés partent dans une corbeille vidée hors du fil audio.

`crates/blast-core/src/mixer.rs` ligne 294

```rust
pub fn apply(&mut self, cmd: Command) -> Option<Arc<Track>> {
    match cmd {
        Command::Load(id, track) => {
            let slot = &mut self.decks[id.index()];
            slot.pos = 0;
            slot.playing = false;
            return slot.track.replace(track);
        }
        Command::Eject(id) => {
            let slot = &mut self.decks[id.index()];
            slot.pos = 0;
            slot.playing = false;
…
```

### [MIX-13] Rendu simple

Produit un bloc de mélange sans séparer antenne et écoute locale.

`crates/blast-core/src/mixer.rs` ligne 551

```rust
pub fn render<F: FnMut(Event)>(&mut self, out: &mut [f32], emit: F) {
    self.render_split(None, out, emit);
}
```

### [MIX-14] Rendu antenne et écoute locale

Produit le mélange antenne (avec micro) et le mélange local (micro seulement si le retour est activé), applique ducking, auto-mix et traitement antenne.

`crates/blast-core/src/mixer.rs` ligne 556

```rust
pub fn render_split<F: FnMut(Event)>(
    &mut self,
    mut local: Option<&mut [f32]>,
    out: &mut [f32],
    mut emit: F,
) {
    let frames = out.len() / 2;
    if let Some(mic) = self.mic.as_mut() {
        let max = self.sample_rate as usize * MIC_MAX_BACKLOG_MS / 1000 + frames;
        let backlog = mic.slots();
        if backlog > max {
            if let Ok(chunk) = mic.read_chunk(backlog - max / 2) {
…
```

### [MIX-15] Traitement du micro

Lit le micro dans sa file, limite le retard, filtre et applique les effets de voix.

`crates/blast-core/src/mixer.rs` ligne 649

```rust
impl MicChain {
    fn build(p: &MicParams, rate: f32) -> Self {
        Self {
            hp: Biquad::highpass(90.0, 0.707, rate),
            low: Biquad::low_shelf(150.0, p.low_db, rate),
            mid: Biquad::peaking(1500.0, 0.8, p.mid_db, rate),
            high: Biquad::high_shelf(6000.0, p.high_db, rate),
        }
    }
}
```

### [MIX-16] Voix des jingles

Les voix numérotées à partir de 1000 sont des jingles : volume propre, arrêt séparé des pads.

`crates/blast-core/src/mixer.rs` ligne 8

```rust
pub const CART_BASE: u16 = 1000;
```

### [MIX-17] Pas du ducking

Variation de gain par échantillon pour que la musique parcoure toute la profondeur du ducking dans le temps réglé (descente ou remontée).

`crates/blast-core/src/mixer.rs` ligne 661

```rust
fn duck_step(depth: f32, secs: f32, sample_rate: u32) -> f32 {
    depth / (sample_rate as f32 * secs.max(0.005))
}
```


## ENG

Moteur audio et périphériques (blast-core)

### [ENG-01] Tampon de travail

Nombre de trames préallouées pour un rappel audio.

`crates/blast-core/src/engine.rs` ligne 10

```rust
const SCRATCH_FRAMES: usize = 4096;
```

### [ENG-02] Capacité de la prise antenne

Secondes de son gardées dans la file vers les encodeurs.

`crates/blast-core/src/engine.rs` ligne 12

```rust
const TAP_SECONDS: usize = 2;
```

### [ENG-03] Erreurs du moteur

Aucune sortie, périphérique introuvable, format non pris en charge ou erreur du pilote audio.

`crates/blast-core/src/engine.rs` ligne 15

```rust
#[derive(Debug)]
pub enum EngineError {
    NoDevice,
    DeviceNotFound(String),
    UnsupportedFormat(String),
    Backend(String),
}
```

### [ENG-04] Conversion d'erreur du pilote

Transforme une erreur cpal en erreur du moteur.

`crates/blast-core/src/engine.rs` ligne 39

```rust
fn backend<E: std::fmt::Display>(e: E) -> EngineError {
    EngineError::Backend(e.to_string())
}
```

### [ENG-05] Sorties audio

Liste les périphériques de sortie du système.

`crates/blast-core/src/engine.rs` ligne 44

```rust
pub fn list_output_devices() -> Result<Vec<String>, EngineError> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    for d in host.output_devices().map_err(backend)? {
        if let Ok(n) = d.name() {
            names.push(n);
        }
    }
    Ok(names)
}
```

### [ENG-06] Sortie par défaut

Nom de la sortie audio par défaut du système.

`crates/blast-core/src/engine.rs` ligne 56

```rust
pub fn default_output_device() -> Option<String> {
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
}
```

### [ENG-07] Choix du micro

Pas de micro, micro par défaut ou micro désigné par son nom.

`crates/blast-core/src/engine.rs` ligne 63

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicInput {
    Off,
    Default,
    Named(String),
}
```

### [ENG-08] Entrées audio

Liste les micros du système.

`crates/blast-core/src/engine.rs` ligne 71

```rust
pub fn list_input_devices() -> Result<Vec<String>, EngineError> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    for d in host.input_devices().map_err(backend)? {
        if let Ok(n) = d.name() {
            names.push(n);
        }
    }
    Ok(names)
}
```

### [ENG-09] Micro par défaut

Nom du micro par défaut du système.

`crates/blast-core/src/engine.rs` ligne 83

```rust
pub fn default_input_device() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.name().ok())
}
```

### [ENG-10] Moteur audio

Ouvre la sortie et le micro, fait tourner le mixeur dans le rappel audio et expose commandes, événements et prise antenne.

`crates/blast-core/src/engine.rs` ligne 90

```rust
pub struct Engine {
    _stream: cpal::Stream,
    _input: Option<cpal::Stream>,
    input_name: Option<String>,
    input_error: Option<String>,
    commands: Sender<Command>,
    events: Receiver<Event>,
    ticks: Receiver<Event>,
    trash: Receiver<Arc<Track>>,
    tap: rtrb::Consumer<f32>,
    sample_rate: u32,
    channels: u16,
…
```

### [ENG-11] Démarrage du moteur

Ouvre la sortie demandée ou celle par défaut, et le micro choisi ; un micro absent n'empêche pas le démarrage.

`crates/blast-core/src/engine.rs` ligne 111

```rust
pub fn start_with(device: Option<&str>, mic: MicInput) -> Result<Self, EngineError> {
    let host = cpal::default_host();
    let device = match device {
        Some(name) => host
            .output_devices()
            .map_err(backend)?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| EngineError::DeviceNotFound(name.to_string()))?,
        None => host.default_output_device().ok_or(EngineError::NoDevice)?,
    };
    let device_name = device.name().unwrap_or_default();
    let supported = device.default_output_config().map_err(backend)?;
…
```

### [ENG-12] Lecture des événements

Renvoie les événements du mixeur ; le tick passe par un canal séparé qui ne garde que le plus récent.

`crates/blast-core/src/engine.rs` ligne 213

```rust
pub fn try_event(&self) -> Option<Event> {
    if let Ok(e) = self.events.try_recv() {
        return Some(e);
    }
    self.ticks.try_recv().ok().map(|t| self.latest_tick(t))
}
```

### [ENG-13] Prise antenne

Vide la file du mélange antenne dans un tampon pour les encodeurs.

`crates/blast-core/src/engine.rs` ligne 232

```rust
pub fn read_master(&mut self, out: &mut Vec<f32>) -> usize {
    let n = self.tap.slots() & !1;
    if n == 0 {
        return 0;
    }
    if let Ok(chunk) = self.tap.read_chunk(n) {
        let (a, b) = chunk.as_slices();
        out.extend_from_slice(a);
        out.extend_from_slice(b);
        chunk.commit_all();
    }
    n
…
```

### [ENG-14] Libération différée

Libère hors du fil audio les titres retirés par le mixeur.

`crates/blast-core/src/engine.rs` ligne 247

```rust
pub fn collect_garbage(&self) {
    while self.trash.try_recv().is_ok() {}
}
```

### [ENG-15] Éléments du rappel audio

Mixeur, files et tampons déplacés dans le rappel de sortie.

`crates/blast-core/src/engine.rs` ligne 253

```rust
struct Parts {
    tap: rtrb::Producer<f32>,
    mixer: Mixer,
    commands: Receiver<Command>,
    events: Sender<Event>,
    ticks: Sender<Event>,
    trash: Sender<Arc<Track>>,
}
```

### [ENG-16] Rappel de sortie

Construit le flux de sortie pour un format d'échantillon et y fait tourner le mixeur.

`crates/blast-core/src/engine.rs` ligne 263

```rust
fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    parts: Parts,
) -> Result<cpal::Stream, EngineError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let Parts {
        mut tap,
        mut mixer,
…
```

### [ENG-17] Flux micro

Flux d'entrée et extrémité de lecture de sa file.

`crates/blast-core/src/engine.rs` ligne 340

```rust
type MicStream = (cpal::Stream, String, rtrb::Consumer<f32>);
```

### [ENG-18] Ouverture du micro

Ouvre le micro choisi et relie sa file au mixeur.

`crates/blast-core/src/engine.rs` ligne 343

```rust
fn open_mic(
    host: &cpal::Host,
    mic: &MicInput,
    out_rate: u32,
) -> Result<Option<MicStream>, EngineError> {
    let device = match mic {
        MicInput::Off => return Ok(None),
        MicInput::Default => match host.default_input_device() {
            Some(d) => d,
            None => return Ok(None),
        },
        MicInput::Named(name) => host
…
```

### [ENG-19] Rappel d'entrée

Convertit le micro en stéréo f32 et le pousse dans la file sans jamais bloquer.

`crates/blast-core/src/engine.rs` ligne 376

```rust
fn build_input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut tx: rtrb::Producer<f32>,
    out_rate: u32,
) -> Result<cpal::Stream, EngineError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = (config.channels as usize).max(1);
    let step = config.sample_rate.0 as f64 / out_rate as f64;
…
```


## CAST

Sorties de diffusion, encodage MP3 et fils d'envoi (blast-cast)

### [CAST-01] Débits proposés

Débits MP3 proposés dans les réglages d'une sortie.

`crates/blast-cast/src/lib.rs` ligne 13

```rust
pub const BITRATES: &[u32] = &[32, 40, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
```

### [CAST-02] Codec de diffusion

Format envoyé au serveur et son type MIME.

`crates/blast-cast/src/lib.rs` ligne 16

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Mp3,
}
```

### [CAST-03] Type de serveur

Icecast 2, Shoutcast v1 ou Shoutcast v2.

`crates/blast-cast/src/lib.rs` ligne 30

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerKind {
    #[default]
    Icecast,
    ShoutcastV1,
    ShoutcastV2,
}
```

### [CAST-04] Sortie de diffusion

Serveur, point de montage ou SID, identifiant, débit, canaux et informations de la station, sans mot de passe.

`crates/blast-cast/src/lib.rs` ligne 50

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: ServerKind,
    pub host: String,
    pub port: u16,
    pub mount: String,
    pub sid: u32,
    pub user: String,
…
```

### [CAST-05] Règles d'une sortie

Point de montage normalisé, nombre de canaux et validation des champs avant enregistrement.

`crates/blast-cast/src/lib.rs` ligne 95

```rust
impl OutputConfig {
    pub fn mount_path(&self) -> String {
        let m = self.mount.trim();
        if m.starts_with('/') {
            m.to_string()
        } else {
            format!("/{m}")
        }
    }

    pub fn channels(&self) -> u8 {
        if self.mono {
…
```

### [CAST-06] Encodeur MP3

Encodeur LAME à débit constant, en stéréo ou en mono.

`crates/blast-cast/src/encoder.rs` ligne 3

```rust
pub struct Mp3Encoder {
    inner: mp3lame_encoder::Encoder,
    mono: bool,
    scratch: Vec<f32>,
    out: Vec<u8>,
}
```

### [CAST-07] Débit LAME

Associe un débit en kb/s à la constante LAME correspondante.

`crates/blast-cast/src/encoder.rs` ligne 11

```rust
fn bitrate(kbps: u32) -> Bitrate {
    match kbps {
        0..=32 => Bitrate::Kbps32,
        33..=40 => Bitrate::Kbps40,
        41..=48 => Bitrate::Kbps48,
        49..=64 => Bitrate::Kbps64,
        65..=80 => Bitrate::Kbps80,
        81..=96 => Bitrate::Kbps96,
        97..=112 => Bitrate::Kbps112,
        113..=128 => Bitrate::Kbps128,
        129..=160 => Bitrate::Kbps160,
        161..=192 => Bitrate::Kbps192,
…
```

### [CAST-08] Encodage

Convertit un bloc stéréo f32 en trames MP3.

`crates/blast-cast/src/encoder.rs` ligne 52

```rust
pub fn encode(&mut self, interleaved: &[f32]) -> Result<&[u8], String> {
    let frames = interleaved.len() / 2;
    self.out.clear();
    self.out
        .reserve(mp3lame_encoder::max_required_buffer_size(frames.max(1)));
    let result = if self.mono {
        self.scratch.clear();
        self.scratch.extend(
            interleaved
                .as_chunks::<2>()
                .0
                .iter()
…
```

### [CAST-09] Délai de reconnexion

Attente de 5 secondes entre deux tentatives de connexion.

`crates/blast-cast/src/caster.rs` ligne 14

```rust
const RETRY_DELAY: Duration = Duration::from_secs(5);
```

### [CAST-10] File de son par sortie

Nombre de blocs gardés en attente par sortie avant d'en jeter.

`crates/blast-cast/src/caster.rs` ligne 16

```rust
const PCM_QUEUE: usize = 128;
```

### [CAST-11] État d'une sortie

Arrêtée, connexion, en direct, reconnexion (avec la raison) ou erreur.

`crates/blast-cast/src/caster.rs` ligne 19

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum OutputStatus {
    Off,
    Connecting,
    Live,
    Retrying(String),
    Error(String),
}
```

### [CAST-12] Changement d'état

Identifiant de la sortie et son nouvel état, remontés à l'interface.

`crates/blast-cast/src/caster.rs` ligne 30

```rust
#[derive(Debug, Clone)]
pub struct StatusEvent {
    pub id: String,
    pub status: OutputStatus,
}
```

### [CAST-13] Poignée de sortie

Canaux de son, de titre et d'arrêt d'un fil d'envoi.

`crates/blast-cast/src/caster.rs` ligne 37

```rust
struct Handle {
    id: String,
    pcm: Sender<Arc<[f32]>>,
    meta: Sender<String>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
```

### [CAST-14] Gestionnaire de diffusion

Un fil par sortie ; répartit le son et le titre en cours, collecte les états.

`crates/blast-cast/src/caster.rs` ligne 46

```rust
pub struct Caster {
    sample_rate: u32,
    outputs: Vec<Handle>,
    status_tx: Sender<StatusEvent>,
    status_rx: Receiver<StatusEvent>,
    metadata: String,
}
```

### [CAST-15] Pilotage des sorties

Démarrer, arrêter sans bloquer, pousser le son et le titre, relever les états.

`crates/blast-cast/src/caster.rs` ligne 55

```rust
impl Caster {
    pub fn new(sample_rate: u32) -> Self {
        let (status_tx, status_rx) = unbounded();
        Self {
            sample_rate,
            outputs: Vec::new(),
            status_tx,
            status_rx,
            metadata: String::new(),
        }
    }
```

### [CAST-16] Fil d'envoi

Encode et envoie le son d'une sortie, se reconnecte en cas de coupure.

`crates/blast-cast/src/caster.rs` ligne 144

```rust
struct Worker {
    cfg: OutputConfig,
    password: String,
    sample_rate: u32,
    pcm: Receiver<Arc<[f32]>>,
    meta: Receiver<String>,
    stop: Arc<AtomicBool>,
    status: Sender<StatusEvent>,
    song: String,
}
```

### [CAST-17] Fin de session d'envoi

Arrêt demandé, coupure à retenter ou erreur définitive (mot de passe refusé).

`crates/blast-cast/src/caster.rs` ligne 156

```rust
enum Outcome {
    Stopped,
    Lost(String),
    Fatal(String),
}
```

### [CAST-18] Boucle d'envoi

Connexion, envoi, mise à jour du titre, reconnexion toutes les 5 secondes, arrêt définitif sur mot de passe refusé.

`crates/blast-cast/src/caster.rs` ligne 163

```rust
impl Worker {
    fn report(&self, status: OutputStatus) {
        let _ = self.status.send(StatusEvent {
            id: self.cfg.id.clone(),
            status,
        });
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
```


## PROT

Protocoles Icecast / Shoutcast et statistiques (blast-cast)

### [PROT-01] Délai de connexion

Temps maximal pour joindre le serveur.

`crates/blast-cast/src/protocol.rs` ligne 9

```rust
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
```

### [PROT-02] Délai d'échange

Temps maximal d'attente d'une réponse ou d'un envoi.

`crates/blast-cast/src/protocol.rs` ligne 11

```rust
const IO_TIMEOUT: Duration = Duration::from_secs(10);
```

### [PROT-03] Agent annoncé

Nom et version de BLAST annoncés aux serveurs.

`crates/blast-cast/src/protocol.rs` ligne 13

```rust
const USER_AGENT: &str = concat!("BLAST-Broadcaster/", env!("CARGO_PKG_VERSION"));
```

### [PROT-04] Erreurs de diffusion

Réseau, authentification refusée, point de montage occupé ou réponse inattendue ; indique si l'erreur est définitive.

`crates/blast-cast/src/protocol.rs` ligne 16

```rust
#[derive(Debug)]
pub enum CastError {
    Connect(String),
    Io(std::io::Error),
    Auth,
    MountInUse,
    Rejected(String),
}
```

### [PROT-05] Nettoyage d'en-tête

Retire retours à la ligne et caractères de contrôle d'une valeur d'en-tête.

`crates/blast-cast/src/protocol.rs` ligne 54

```rust
fn clean(s: &str) -> String {
    s.chars().filter(|c| *c != '\r' && *c != '\n').collect()
}
```

### [PROT-06] Encodage d'URL

Encode un texte pour une requête de mise à jour du titre.

`crates/blast-cast/src/protocol.rs` ligne 59

```rust
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
```

### [PROT-07] Connexion TCP

Résout l'adresse et se connecte avec délais.

`crates/blast-cast/src/protocol.rs` ligne 72

```rust
fn connect(host: &str, port: u16) -> Result<TcpStream, CastError> {
    let addrs: Vec<_> = (host.trim(), port)
        .to_socket_addrs()
        .map_err(|e| CastError::Connect(format!("adresse {host}:{port} inconnue : {e}")))?
        .collect();
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(s) => {
                s.set_read_timeout(Some(IO_TIMEOUT))?;
                s.set_write_timeout(Some(IO_TIMEOUT))?;
                s.set_nodelay(true)?;
…
```

### [PROT-08] Lecture d'en-têtes

Lit la réponse jusqu'à la fin des en-têtes.

`crates/blast-cast/src/protocol.rs` ligne 96

```rust
fn read_until(stream: &mut TcpStream, end: &[u8], limit: usize) -> Result<Vec<u8>, CastError> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while buf.len() < limit {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                buf.push(byte[0]);
                if buf.ends_with(end) {
                    break;
                }
            }
…
```

### [PROT-09] Statut HTTP

Extrait le code de la ligne de statut.

`crates/blast-cast/src/protocol.rs` ligne 115

```rust
fn http_status(head: &[u8]) -> (u16, String) {
    let text = String::from_utf8_lossy(head);
    let line = text.lines().next().unwrap_or("").trim().to_string();
    let code = line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (code, line)
}
```

### [PROT-10] Authentification Basic

Construit l'en-tête Authorization.

`crates/blast-cast/src/protocol.rs` ligne 127

```rust
fn basic_auth(user: &str, password: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
}
```

### [PROT-11] Informations audio

Débit, canaux et fréquence annoncés au serveur.

`crates/blast-cast/src/protocol.rs` ligne 132

```rust
fn audio_info(cfg: &OutputConfig, sample_rate: u32) -> String {
    format!(
        "ice-samplerate={sample_rate};ice-bitrate={};ice-channels={}",
        cfg.bitrate,
        cfg.channels()
    )
}
```

### [PROT-12] Ouverture de la source

Ouvre la connexion source selon le type de serveur.

`crates/blast-cast/src/protocol.rs` ligne 141

```rust
pub fn open_source(
    cfg: &OutputConfig,
    password: &str,
    sample_rate: u32,
) -> Result<TcpStream, CastError> {
    match cfg.kind {
        ServerKind::Icecast => open_icecast(cfg, password, sample_rate),
        ServerKind::ShoutcastV1 | ServerKind::ShoutcastV2 => open_shoutcast(cfg, password),
    }
}
```

### [PROT-13] Source Icecast

Requête PUT avec Expect: 100-continue et authentification Basic.

`crates/blast-cast/src/protocol.rs` ligne 153

```rust
fn open_icecast(
    cfg: &OutputConfig,
    password: &str,
    sample_rate: u32,
) -> Result<TcpStream, CastError> {
    let mut s = connect(&cfg.host, cfg.port)?;
    let user = if cfg.user.trim().is_empty() {
        "source"
    } else {
        cfg.user.trim()
    };
    let head = format!(
…
```

### [PROT-14] Source Shoutcast

Mot de passe sur le port + 1 (v1) ou « motdepasse:#sid » (v2), puis attente de OK2.

`crates/blast-cast/src/protocol.rs` ligne 210

```rust
fn open_shoutcast(cfg: &OutputConfig, password: &str) -> Result<TcpStream, CastError> {
    let mut s = connect(&cfg.host, cfg.port.saturating_add(1))?;
    let pass = match cfg.kind {
        ServerKind::ShoutcastV2 => format!("{}:#{}", clean(password), cfg.sid),
        _ => clean(password),
    };
    s.write_all(format!("{pass}\r\n").as_bytes())?;
    let resp = read_until(&mut s, b"\r\n", 1024)?;
    let line = String::from_utf8_lossy(&resp).trim().to_string();
    if !line.starts_with("OK") {
        return if line.to_lowercase().contains("password") || line.is_empty() {
            Err(CastError::Auth)
…
```

### [PROT-15] Titre en cours

Envoie « Artiste - Titre » par /admin/metadata (Icecast) ou /admin.cgi (Shoutcast).

`crates/blast-cast/src/protocol.rs` ligne 239

```rust
pub fn update_metadata(cfg: &OutputConfig, password: &str, song: &str) -> Result<(), CastError> {
    let song = percent_encode(song);
    let (path, auth) = match cfg.kind {
        ServerKind::Icecast => {
            let user = if cfg.user.trim().is_empty() {
                "source"
            } else {
                cfg.user.trim()
            };
            (
                format!(
                    "/admin/metadata?mount={}&mode=updinfo&song={song}",
…
```

### [PROT-16] Auditeurs

Nombre d'auditeurs en cours et pic.

`crates/blast-cast/src/protocol.rs` ligne 293

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listeners {
    pub current: u32,
    pub peak: u32,
}
```

### [PROT-17] Requête de statistiques

Requête GET avec authentification vers la page de statistiques.

`crates/blast-cast/src/protocol.rs` ligne 300

```rust
fn http_get(cfg: &OutputConfig, path: &str, auth: Option<String>) -> Result<String, CastError> {
    let mut s = connect(&cfg.host, cfg.port)?;
    let mut req = format!(
        "GET {path} HTTP/1.0\r\nHost: {}:{}\r\nUser-Agent: Mozilla/5.0 ({USER_AGENT})\r\n",
        clean(&cfg.host),
        cfg.port
    );
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: Basic {a}\r\n"));
    }
    req.push_str("Connection: close\r\n\r\n");
    s.write_all(req.as_bytes())?;
…
```

### [PROT-18] Lecture JSON minimale

Extrait la valeur numérique d'une clé dans une réponse JSON.

`crates/blast-cast/src/protocol.rs` ligne 333

```rust
fn json_number(text: &str, key: &str) -> Option<u32> {
    let pat = format!("\"{key}\"");
    let at = text.find(&pat)? + pat.len();
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}
```

### [PROT-19] Statistiques Icecast

Lit status-json.xsl et retient le point de montage de la sortie.

`crates/blast-cast/src/protocol.rs` ligne 342

```rust
pub fn parse_icecast_status(body: &str, mount: &str) -> Option<Listeners> {
    for chunk in body.split("\"listenurl\"").skip(1) {
        let url_end = chunk.find(',').unwrap_or(chunk.len());
        let url = &chunk[..url_end];
        let matches = url
            .trim()
            .trim_start_matches(':')
            .trim()
            .trim_matches('"')
            .ends_with(mount);
        if !matches {
            continue;
…
```

### [PROT-20] Statistiques Shoutcast v1

Lit la page 7.html.

`crates/blast-cast/src/protocol.rs` ligne 371

```rust
pub fn parse_shoutcast_v1(body: &str) -> Option<Listeners> {
    let inner = body.split("<body>").nth(1)?.split("</body>").next()?;
    let parts: Vec<&str> = inner.split(',').collect();
    Some(Listeners {
        current: parts.first()?.trim().parse().ok()?,
        peak: parts.get(2)?.trim().parse().unwrap_or(0),
    })
}
```

### [PROT-21] Statistiques Shoutcast v2

Lit stats?sid=&json=1.

`crates/blast-cast/src/protocol.rs` ligne 381

```rust
pub fn parse_shoutcast_v2(body: &str) -> Option<Listeners> {
    Some(Listeners {
        current: json_number(body, "currentlisteners")?,
        peak: json_number(body, "peaklisteners").unwrap_or(0),
    })
}
```

### [PROT-22] Relevé des auditeurs

Interroge le serveur selon son type et renvoie les auditeurs.

`crates/blast-cast/src/protocol.rs` ligne 389

```rust
pub fn fetch_listeners(cfg: &OutputConfig) -> Result<Listeners, CastError> {
    let parsed = match cfg.kind {
        ServerKind::Icecast => {
            let body = http_get(cfg, "/status-json.xsl", None)?;
            parse_icecast_status(&body, &cfg.mount_path())
        }
        ServerKind::ShoutcastV1 => parse_shoutcast_v1(&http_get(cfg, "/7.html", None)?),
        ServerKind::ShoutcastV2 => parse_shoutcast_v2(&http_get(
            cfg,
            &format!("/stats?sid={}&json=1", cfg.sid),
            None,
        )?),
…
```

### [PROT-23] Test de connexion

Ouvre une source, envoie un court silence puis referme.

`crates/blast-cast/src/protocol.rs` ligne 406

```rust
pub fn test_output(
    cfg: &OutputConfig,
    password: &str,
    sample_rate: u32,
) -> Result<String, CastError> {
    let s = open_source(cfg, password, sample_rate)?;
    let _ = s.shutdown(std::net::Shutdown::Both);
    Ok(match cfg.kind {
        ServerKind::Icecast => format!(
            "Connexion acceptée par Icecast sur {}:{}{}",
            cfg.host,
            cfg.port,
…
```


## AURA

Pilotage d'OBS par obs-websocket (blast-aura)

### [AURA-01] Réglages OBS

Adresse, port, connexion au démarrage et source texte « Now Playing ».

`crates/blast-aura/src/lib.rs` ligne 14

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuraSettings {
    pub host: String,
    pub port: u16,
    pub auto_connect: bool,
    pub now_playing_source: String,
}
```

### [AURA-02] Actions OBS élémentaires

Changer de scène, lancer ou arrêter stream et enregistrement, afficher une source, écrire un texte.

`crates/blast-aura/src/lib.rs` ligne 41

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AuraAction {
    SetScene(String),
    StartStream,
    StopStream,
    StartRecord,
    StopRecord,
    SetSourceVisible {
        scene: String,
        source: String,
        visible: bool,
    },
…
```

### [AURA-03] Informations OBS

Versions d'OBS et d'obs-websocket, plateforme.

`crates/blast-aura/src/lib.rs` ligne 60

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObsInfo {
    pub obs_version: String,
    pub websocket_version: String,
    pub platform: String,
}
```

### [AURA-04] Erreurs OBS

Connexion impossible, authentification refusée ou réponse inattendue.

`crates/blast-aura/src/lib.rs` ligne 68

```rust
#[derive(Debug)]
pub enum AuraError {
    Connect(String),
    Protocol(String),
    PasswordRequired,
    AuthFailed,
}
```

### [AURA-05] Authentification obs-websocket

SHA-256 du mot de passe et du sel, puis du défi, encodés en base64.

`crates/blast-aura/src/lib.rs` ligne 90

```rust
pub fn auth_response(password: &str, salt: &str, challenge: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = b64.encode(Sha256::digest(format!("{password}{salt}").as_bytes()));
    b64.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}
```

### [AURA-06] Lecture d'un message

Lit le prochain message texte JSON de la WebSocket.

`crates/blast-aura/src/lib.rs` ligne 97

```rust
pub(crate) fn read_json(ws: &mut tungstenite::WebSocket<TcpStream>) -> Result<Value, AuraError> {
    loop {
        match ws.read() {
            Ok(Message::Text(t)) => {
                return serde_json::from_str(t.as_str())
                    .map_err(|e| AuraError::Protocol(e.to_string()))
            }
            Ok(Message::Close(frame)) => {
                let code: u16 = frame.as_ref().map(|f| f.code.into()).unwrap_or(0);
                return Err(if code == 4009 {
                    AuraError::AuthFailed
                } else {
…
```

### [AURA-07] Test de connexion OBS

Se connecte, s'authentifie et renvoie les versions.

`crates/blast-aura/src/lib.rs` ligne 122

```rust
pub fn test_connection(
    settings: &AuraSettings,
    password: Option<&str>,
) -> Result<ObsInfo, AuraError> {
    let addr = (settings.host.trim(), settings.port)
        .to_socket_addrs()
        .map_err(|e| AuraError::Connect(e.to_string()))?
        .next()
        .ok_or_else(|| AuraError::Connect("adresse inconnue".into()))?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(4))
        .map_err(|e| AuraError::Connect(e.to_string()))?;
    stream
…
```

### [AURA-08] Abonnement aux événements

Masque de tous les événements obs-websocket (2047).

`crates/blast-aura/src/client.rs` ligne 19

```rust
const EVENTS_ALL: u32 = 2047;
```

### [AURA-09] Sources audio OBS

Types de sources traités comme audio.

`crates/blast-aura/src/client.rs` ligne 23

```rust
const AUDIO_KINDS: &[&str] = &[
    "pulse_input_capture",
    "pulse_output_capture",
    "pipewire-audio-capture-input",
    "pipewire-audio-capture-output",
    "alsa_input_capture",
    "jack_output_capture",
    "wasapi_input_capture",
    "wasapi_output_capture",
    "wasapi_process_output_capture",
    "coreaudio_input_capture",
    "coreaudio_output_capture",
…
```

### [AURA-10] Commandes OBS

Scène, stream, enregistrement, pause, replay, capture, volume, muet, texte et rafraîchissement.

`crates/blast-aura/src/client.rs` ligne 39

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ObsCommand {
    SetScene(String),
    ToggleStream,
    ToggleRecord,
    ToggleRecordPause,
    SaveReplay,
    Screenshot(PathBuf),
    SetVolume { input: String, mul: f32 },
    ToggleMute(String),
    SetText { source: String, text: String },
    Refresh,
…
```

### [AURA-11] Source audio OBS

Nom et type d'une source audio.

`crates/blast-aura/src/client.rs` ligne 54

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObsInput {
    pub name: String,
    pub kind: String,
}
```

### [AURA-12] Événements OBS

Connexion, scènes, stream, enregistrement, sources, volume, informations et erreurs.

`crates/blast-aura/src/client.rs` ligne 61

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ObsEvent {
    Connecting,
    Connected {
        obs_version: String,
        websocket_version: String,
    },
    Disconnected {
        reason: String,
        retrying: bool,
    },
    Scenes {
…
```

### [AURA-13] Client OBS

Fil de connexion persistant, piloté par commandes, qui remonte des événements.

`crates/blast-aura/src/client.rs` ligne 100

```rust
pub struct AuraClient {
    commands: Sender<ObsCommand>,
    events: Receiver<ObsEvent>,
    stop: Arc<AtomicBool>,
}
```

### [AURA-14] Fin de session OBS

Arrêt demandé ou coupure.

`crates/blast-aura/src/client.rs` ligne 138

```rust
enum End {
    Stopped,
    Lost(String),
    Fatal(String),
}
```

### [AURA-15] Boucle de connexion

Se reconnecte avec un délai croissant de 2 à 30 secondes.

`crates/blast-aura/src/client.rs` ligne 145

```rust
fn run(
    settings: AuraSettings,
    password: Option<String>,
    commands: Receiver<ObsCommand>,
    events: Sender<ObsEvent>,
    stop: Arc<AtomicBool>,
) {
    let mut delay = Duration::from_secs(2);
    while !stop.load(Ordering::Relaxed) {
        let _ = events.send(ObsEvent::Connecting);
        let started = Instant::now();
        match session(&settings, password.as_deref(), &commands, &events, &stop) {
…
```

### [AURA-16] Lecture non bloquante

Reconnaît une attente sans données sur la socket.

`crates/blast-aura/src/client.rs` ligne 189

```rust
fn would_block(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(io) if matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
}
```

### [AURA-17] Session OBS

État d'une connexion : requêtes en attente par requestId et sources suivies.

`crates/blast-aura/src/client.rs` ligne 194

```rust
struct Session<'a> {
    ws: WebSocket<TcpStream>,
    events: &'a Sender<ObsEvent>,
    seq: u64,
    current_scene: Option<String>,
    audio_inputs: HashSet<String>,
    pending_input: HashMap<String, String>,
}
```

### [AURA-18] Traitement de session

Envoi des requêtes, lecture des réponses et des événements, conversion en ObsEvent.

`crates/blast-aura/src/client.rs` ligne 204

```rust
impl Session<'_> {
    fn request(
        &mut self,
        kind: &str,
        id_prefix: &str,
        data: Value,
    ) -> Result<(), tungstenite::Error> {
        self.seq += 1;
        let id = format!("{id_prefix}#{}", self.seq);
        if let Some(name) = data["inputName"].as_str() {
            if matches!(id_prefix, "mute" | "vol") {
                self.pending_input.insert(id.clone(), name.to_string());
…
```

### [AURA-19] Poignée de main

Hello, Identify avec authentification et attente de Identified.

`crates/blast-aura/src/client.rs` ligne 434

```rust
fn handshake(
    settings: &AuraSettings,
    password: Option<&str>,
) -> Result<(WebSocket<TcpStream>, String, String), AuraError> {
    let addr = (settings.host.trim(), settings.port)
        .to_socket_addrs()
        .map_err(|e| AuraError::Connect(e.to_string()))?
        .next()
        .ok_or_else(|| AuraError::Connect("adresse inconnue".into()))?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(4))
        .map_err(|e| AuraError::Connect(e.to_string()))?;
    stream
…
```

### [AURA-20] Session complète

Ouvre la WebSocket, s'identifie et fait tourner la session.

`crates/blast-aura/src/client.rs` ligne 495

```rust
fn session(
    settings: &AuraSettings,
    password: Option<&str>,
    commands: &Receiver<ObsCommand>,
    events: &Sender<ObsEvent>,
    stop: &AtomicBool,
) -> End {
    let (ws, obs_version, websocket_version) = match handshake(settings, password) {
        Ok(v) => v,
        Err(e @ (AuraError::AuthFailed | AuraError::PasswordRequired)) => {
            return End::Fatal(e.to_string())
        }
…
```

### [AURA-21] Pause de la boucle OBS

Attente quand la socket non bloquante n'a rien à lire ; évite les lectures à délai, dont l'état est indéterminé sous Windows après expiration.

`crates/blast-aura/src/client.rs` ligne 21

```rust
const IDLE: Duration = Duration::from_millis(15);
```


## LIB

Bibliothèque SQLite, historique et règles de séparation (blast-library)

### [LIB-01] Version du schéma

Numéro de version de la base (2 : type des passages et des titres), enregistré dans la table meta.

`crates/blast-library/src/sqlite.rs` ligne 10

```rust
const SCHEMA_VERSION: i64 = 3;
```

### [LIB-02] Schéma SQLite

Tables tracks, historique_diffusion et meta, avec leurs index.

`crates/blast-library/src/sqlite.rs` ligne 13

```rust
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### [LIB-03] Titre en bibliothèque

Chemin, titre, artiste, album, genre, année, durée et type enregistrés.

`crates/blast-library/src/lib.rs` ligne 58

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackMeta {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<i32>,
    pub duration: Option<f64>,
    pub kind: Kind,
    pub bpm: Option<f32>,
    pub mtime: Option<i64>,
…
```

### [LIB-04] Règles de séparation

Activation et délais minimaux entre deux passages d'un même artiste ou d'un même titre.

`crates/blast-library/src/lib.rs` ligne 100

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeparationRules {
    pub enabled: bool,
    pub artist_minutes: u32,
    pub title_minutes: u32,
}
```

### [LIB-05] Règle enfreinte

Artiste ou titre passé trop récemment, avec le délai restant ; décrite en français pour la playlist.

`crates/blast-library/src/lib.rs` ligne 119

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum Violation {
    Title { minutes_ago: u32, limit: u32 },
    Artist { minutes_ago: u32, limit: u32 },
}
```

### [LIB-06] Ligne d'historique

Un passage : début, type, titre, artiste, durée diffusée, diffusion en direct et auditeurs.

`crates/blast-library/src/lib.rs` ligne 140

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryRow {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub kind: Kind,
    pub started_at: i64,
    pub duration_played: Option<f64>,
    pub on_air: bool,
    pub listeners: Option<u32>,
}
```

### [LIB-07] Statistiques

Nombre de titres connus et de passages enregistrés.

`crates/blast-library/src/lib.rs` ligne 154

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LibraryStats {
    pub tracks: i64,
    pub plays: i64,
}
```

### [LIB-08] Base SQLite

Implémentation SQLite de l'interface de stockage, en mode WAL : titres connus et historique de diffusion.

`crates/blast-library/src/sqlite.rs` ligne 64

```rust
pub struct SqliteStore {
    conn: Connection,
    label: String,
}
```

### [LIB-09] Ouverture de la base

Ouvre ou crée la base et applique le schéma.

`crates/blast-library/src/sqlite.rs` ligne 71

```rust
pub fn open(path: &Path) -> Result<Self, StoreError> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(path).map_err(err)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(err)?;
    conn.busy_timeout(std::time::Duration::from_secs(3))
        .map_err(err)?;
    Self::init(conn, format!("SQLite ({})", path.display()))
}
```

### [LIB-10] Enregistrement des titres

Ajoute ou met à jour les titres scannés.

`crates/blast-library/src/sqlite.rs` ligne 184

```rust
fn upsert_tracks(&mut self, tracks: &[TrackMeta], now: i64) -> Result<usize, StoreError> {
    let tx = self.conn.transaction().map_err(err)?;
    {
        let mut st = tx
            .prepare_cached(
                "INSERT INTO tracks(path, title, artist, album, genre, year, duration, kind, bpm, mtime, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(path) DO UPDATE SET
                    title = excluded.title, artist = excluded.artist, album = excluded.album,
                    genre = excluded.genre, year = excluded.year, duration = excluded.duration,
                    kind = excluded.kind, bpm = COALESCE(excluded.bpm, tracks.bpm),
                    mtime = COALESCE(excluded.mtime, tracks.mtime)",
…
```

### [LIB-11] Début de passage

Inscrit un passage dans historique_diffusion et renvoie son identifiant.

`crates/blast-library/src/sqlite.rs` ligne 220

```rust
fn record_play(&mut self, p: &NewPlay) -> Result<i64, StoreError> {
    let track_id: Option<i64> = self
        .conn
        .query_row(
            "SELECT id FROM tracks WHERE path = ?1",
            params![p.path],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    self.conn
        .execute(
…
```

### [LIB-12] Fin de passage

Enregistre la durée réellement écoutée.

`crates/blast-library/src/sqlite.rs` ligne 259

```rust
fn finish_play(&mut self, id: i64, duration_played: f64) -> Result<(), StoreError> {
    self.conn
        .execute(
            "UPDATE historique_diffusion SET duration_played = ?2 WHERE id = ?1",
            params![id, duration_played],
        )
        .map_err(err)?;
    Ok(())
}
```

### [LIB-13] Recherche des derniers passages en SQL

Dernier passage d'un titre ou d'un artiste depuis une date, morceaux seulement, par requêtes indexées.

`crates/blast-library/src/sqlite.rs` ligne 147

```rust
impl PlayLookup for SqliteStore {
    fn last_title_play(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        since: i64,
    ) -> Result<Option<i64>, StoreError> {
        self.conn
            .query_row(
                "SELECT MAX(started_at) FROM historique_diffusion
                 WHERE started_at >= ?1 AND kind = 'morceau'
…
```

### [LIB-14] Derniers passages *(retiré)*

Renvoyait les derniers passages ; remplacé par les tampons mémoire [LIB-36].

### [LIB-15] Passages sur une période

Renvoie les passages entre deux instants, éventuellement d'une seule catégorie.

`crates/blast-library/src/sqlite.rs` ligne 270

```rust
fn history_between(
    &self,
    from: i64,
    to: i64,
    kind: Option<Kind>,
    limit: u32,
) -> Result<Vec<HistoryRow>, StoreError> {
    let mut st = self
        .conn
        .prepare_cached(
            "SELECT id, path, title, artist, kind, started_at, duration_played, on_air, listeners
             FROM historique_diffusion
…
```

### [LIB-16] Purge

Supprime les passages plus anciens qu'une date.

`crates/blast-library/src/sqlite.rs` ligne 293

```rust
fn purge_older_than(&mut self, cutoff: i64) -> Result<usize, StoreError> {
    self.conn
        .execute(
            "DELETE FROM historique_diffusion WHERE started_at < ?1",
            params![cutoff],
        )
        .map_err(err)
}
```

### [LIB-17] Effacement

Vide tout l'historique.

`crates/blast-library/src/sqlite.rs` ligne 303

```rust
fn clear_history(&mut self) -> Result<usize, StoreError> {
    self.conn
        .execute("DELETE FROM historique_diffusion", [])
        .map_err(err)
}
```

### [LIB-18] Calcul des statistiques

Compte les titres et les passages.

`crates/blast-library/src/sqlite.rs` ligne 351

```rust
fn stats(&self) -> Result<LibraryStats, StoreError> {
    let count = |sql: &str| self.conn.query_row(sql, [], |r| r.get(0)).map_err(err);
    Ok(LibraryStats {
        tracks: count("SELECT COUNT(*) FROM tracks")?,
        plays: count("SELECT COUNT(*) FROM historique_diffusion")?,
    })
}
```

### [LIB-19] Champ CSV

Met un champ entre guillemets si nécessaire.

`crates/blast-library/src/lib.rs` ligne 233

```rust
fn csv_field(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
```

### [LIB-20] Export CSV

Produit l'historique au format CSV avec le type et des dates lisibles.

`crates/blast-library/src/lib.rs` ligne 242

```rust
pub fn history_csv(rows: &[HistoryRow], format_time: impl Fn(i64) -> String) -> String {
    let mut out = String::from(
        "\u{feff}date;type;artiste;titre;duree_diffusee_s;antenne;auditeurs;fichier\r\n",
    );
    for r in rows.iter().rev() {
        out.push_str(&format!(
            "{};{};{};{};{};{};{};{}\r\n",
            format_time(r.started_at),
            r.kind.as_str(),
            csv_field(&r.artist),
            csv_field(&r.title),
            r.duration_played
…
```

### [LIB-21] Type de passage

Morceau, jingle ou pub ; morceau par défaut.

`crates/blast-library/src/lib.rs` ligne 16

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Morceau,
    Jingle,
    Pub,
}
```

### [LIB-22] Erreur de stockage

Erreur d'une base, quelle qu'elle soit, décrite en texte.

`crates/blast-library/src/lib.rs` ligne 46

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(pub String);
```

### [LIB-23] Nouveau passage

Ce qu'on enregistre au début d'un passage : fichier, titre, artiste, type, heure, diffusion en direct et auditeurs.

`crates/blast-library/src/lib.rs` ligne 88

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct NewPlay {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub kind: Kind,
    pub started_at: i64,
    pub on_air: bool,
    pub listeners: Option<u32>,
}
```

### [LIB-24] Recherche des derniers passages

Ce que les règles de séparation demandent à une source : dernier passage d'un titre, dernier passage d'un artiste.

`crates/blast-library/src/lib.rs` ligne 161

```rust
pub trait PlayLookup {
    fn last_title_play(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        since: i64,
    ) -> Result<Option<i64>, StoreError>;

    fn last_artist_play(&self, artist: &str, since: i64) -> Result<Option<i64>, StoreError>;
}
```

### [LIB-25] Interface de stockage

Opérations attendues d'une base d'historique ; SQLite aujourd'hui, MariaDB ou MySQL pourront s'y brancher.

`crates/blast-library/src/lib.rs` ligne 174

```rust
pub trait HistoryStore: PlayLookup + Send {
    fn backend(&self) -> String;
    fn upsert_tracks(&mut self, tracks: &[TrackMeta], now: i64) -> Result<usize, StoreError>;
    fn record_play(&mut self, play: &NewPlay) -> Result<i64, StoreError>;
    fn finish_play(&mut self, id: i64, duration_played: f64) -> Result<(), StoreError>;
    fn history_between(
        &self,
        from: i64,
        to: i64,
        kind: Option<Kind>,
        limit: u32,
    ) -> Result<Vec<HistoryRow>, StoreError>;
…
```

### [LIB-26] Application des règles

Vérifie titre puis artiste sur n'importe quelle source (tampons mémoire ou base).

`crates/blast-library/src/lib.rs` ligne 194

```rust
pub fn check_rules<L: PlayLookup + ?Sized>(
    source: &L,
    path: &str,
    title: &str,
    artist: &str,
    now: i64,
    rules: &SeparationRules,
) -> Result<Option<Violation>, StoreError> {
    if !rules.enabled {
        return Ok(None);
    }
    let ago = |t: i64| ((now - t).max(0) / 60) as u32;
…
```

### [LIB-27] Index de la version 2

Index par type et par date de l'historique, et par type des titres.

`crates/blast-library/src/sqlite.rs` ligne 54

```rust
const INDEXES_V2: &str = "
CREATE INDEX IF NOT EXISTS idx_hist_kind ON historique_diffusion(kind, started_at);
```

### [LIB-28] Migration

Ajoute la colonne type aux bases de version 1, sans perte de données.

`crates/blast-library/src/sqlite.rs` ligne 107

```rust
fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let columns = [
        ("tracks", "kind", "TEXT NOT NULL DEFAULT 'morceau'"),
        (
            "historique_diffusion",
            "kind",
            "TEXT NOT NULL DEFAULT 'morceau'",
        ),
        ("tracks", "bpm", "REAL"),
        ("tracks", "mtime", "INTEGER"),
    ];
    for (table, column, decl) in columns {
…
```

### [LIB-29] Tampon circulaire

Passages les plus récents en premier ; sait s'il contient tout l'historique ou seulement sa fin.

`crates/blast-library/src/cache.rs` ligne 5

```rust
#[derive(Debug, Clone)]
struct Ring {
    rows: VecDeque<HistoryRow>,
    complete: bool,
}
```

### [LIB-30] Tampons mémoire

Tampon global et un tampon par catégorie, de même taille.

`crates/blast-library/src/cache.rs` ligne 33

```rust
#[derive(Debug, Clone)]
pub struct HistoryCache {
    cap: usize,
    all: Ring,
    by_kind: [Ring; 3],
}
```

### [LIB-31] Pré-remplissage

Charge un tampon depuis la base au démarrage.

`crates/blast-library/src/cache.rs` ligne 55

```rust
pub fn prefill(&mut self, kind: Option<Kind>, newest_first: Vec<HistoryRow>) {
    let ring = match kind {
        None => &mut self.all,
        Some(k) => &mut self.by_kind[k.index()],
    };
    ring.complete = newest_first.len() < self.cap;
    ring.rows = newest_first.into_iter().take(self.cap).collect();
}
```

### [LIB-32] Ajout d'un passage

Met à jour le tampon global et celui de la catégorie.

`crates/blast-library/src/cache.rs` ligne 65

```rust
pub fn push(&mut self, row: HistoryRow) {
    let k = row.kind.index();
    self.by_kind[k].push(row.clone(), self.cap);
    self.all.push(row, self.cap);
}
```

### [LIB-33] Durée d'un passage

Complète la durée diffusée dans tous les tampons.

`crates/blast-library/src/cache.rs` ligne 72

```rust
pub fn finish(&mut self, id: i64, duration_played: f64) {
    for ring in std::iter::once(&mut self.all).chain(self.by_kind.iter_mut()) {
        if let Some(r) = ring.rows.iter_mut().find(|r| r.id == id) {
            r.duration_played = Some(duration_played);
        }
    }
}
```

### [LIB-34] Couverture

Indique si les tampons contiennent tout ce qui s'est passé depuis une date.

`crates/blast-library/src/cache.rs` ligne 88

```rust
pub fn covers(&self, kind: Kind, since: i64) -> bool {
    self.by_kind[kind.index()].covers(since)
}
```

### [LIB-35] Recherche en mémoire

Dernier passage d'un titre ou d'un artiste dans le tampon des morceaux, sans requête.

`crates/blast-library/src/cache.rs` ligne 111

```rust
impl PlayLookup for HistoryCache {
    fn last_title_play(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        since: i64,
    ) -> Result<Option<i64>, StoreError> {
        Ok(self
            .songs(since)
            .find(|r| {
                r.path == path
…
```

### [LIB-36] Historique

Base et tampons mémoire réunis derrière un point d'entrée unique.

`crates/blast-library/src/history.rs` ligne 9

```rust
pub struct History {
    store: Box<dyn HistoryStore>,
    cache: HistoryCache,
    offline_id: i64,
    store_lookups: Cell<u64>,
}
```

### [LIB-37] Ouverture de l'historique

Ouvre l'historique sur une base et pré-remplit les tampons ; la taille se change à chaud.

`crates/blast-library/src/history.rs` ligne 18

```rust
pub fn new(store: Box<dyn HistoryStore>, capacity: usize) -> Result<Self, StoreError> {
    let mut h = Self {
        store,
        cache: HistoryCache::new(capacity),
        offline_id: 0,
        store_lookups: Cell::new(0),
    };
    h.reload()?;
    Ok(h)
}
```

### [LIB-38] Enregistrement d'un passage

Écrit en base et dans les tampons ; si la base échoue, le passage reste en mémoire avec un identifiant provisoire.

`crates/blast-library/src/history.rs` ligne 60

```rust
pub fn record(&mut self, play: NewPlay) -> (i64, Option<StoreError>) {
    let (id, error) = match self.store.record_play(&play) {
        Ok(id) => (id, None),
        Err(e) => {
            self.offline_id -= 1;
            (self.offline_id, Some(e))
        }
    };
    self.cache.push(HistoryRow {
        id,
        path: play.path,
        title: play.title,
…
```

### [LIB-39] Règles de séparation

En mémoire quand les tampons couvrent la fenêtre des règles, sinon en base (règles de plusieurs jours).

`crates/blast-library/src/history.rs` ligne 92

```rust
pub fn check(
    &self,
    path: &str,
    title: &str,
    artist: &str,
    now: i64,
    rules: &SeparationRules,
) -> Result<Option<Violation>, StoreError> {
    if self.cache.covers(Kind::Morceau, now - rules_window(rules)) {
        check_rules(&self.cache, path, title, artist, now, rules)
    } else {
        self.store_lookups.set(self.store_lookups.get() + 1);
…
```

### [LIB-40] Historique pour l'export

Lit dans la base tout l'historique d'une période et d'une catégorie, sans limite.

`crates/blast-library/src/history.rs` ligne 117

```rust
pub fn between(
    &self,
    from: i64,
    to: i64,
    kind: Option<Kind>,
) -> Result<Vec<HistoryRow>, StoreError> {
    self.store.history_between(from, to, kind, u32::MAX)
}
```

### [LIB-41] Titre de la bibliothèque

Ce que les grilles savent d'un titre : tags, BPM, durée, type, nombre de passages et dernier passage.

`crates/blast-library/src/lib.rs` ligne 73

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LibraryTrack {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub genre: String,
    pub year: Option<i32>,
    pub bpm: Option<f32>,
    pub duration: Option<f64>,
    pub kind: Kind,
    pub play_count: i64,
    pub last_played: Option<i64>,
…
```

### [LIB-42] Liste des titres

Tous les titres connus de la base, pour les grilles.

`crates/blast-library/src/sqlite.rs` ligne 310

```rust
fn tracks(&self) -> Result<Vec<LibraryTrack>, StoreError> {
    let mut st = self
        .conn
        .prepare_cached(
            "SELECT path, title, artist, genre, year, bpm, duration, kind, play_count, last_played
             FROM tracks ORDER BY path",
        )
        .map_err(err)?;
    let rows = st
        .query_map([], |r| {
            let kind: String = r.get(7)?;
            Ok(LibraryTrack {
…
```

### [LIB-43] Dates de modification

Date de modification enregistrée pour chaque fichier, pour ne relire que ce qui a changé.

`crates/blast-library/src/sqlite.rs` ligne 339

```rust
fn track_mtimes(&self) -> Result<std::collections::HashMap<String, i64>, StoreError> {
    let mut st = self
        .conn
        .prepare_cached("SELECT path, mtime FROM tracks WHERE mtime IS NOT NULL")
        .map_err(err)?;
    let rows = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(err)?;
    rows.collect::<Result<_, _>>().map_err(err)
}
```

### [LIB-44] Taille du planning

7 jours de 24 heures.

`crates/blast-library/src/rotation.rs` ligne 7

```rust
pub const SCHEDULE_SLOTS: usize = 7 * 24;
```

### [LIB-45] Plafond d'ancienneté

Au-delà d'une semaine sans passage, tous les titres ont le même poids.

`crates/blast-library/src/rotation.rs` ligne 9

```rust
const STALE_HOURS: f64 = 168.0;
```

### [LIB-46] Règles d'une catégorie

Types, mots de genre, années, BPM, dossiers et playlists M3U ; un critère vide n'impose rien, une année ou un BPM à 0 non plus.

`crates/blast-library/src/rotation.rs` ligne 12

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoryRule {
    pub kinds: Vec<Kind>,
    pub genres: Vec<String>,
    pub year_min: Option<i32>,
    pub year_max: Option<i32>,
    pub bpm_min: Option<f32>,
    pub bpm_max: Option<f32>,
    pub folders: Vec<String>,
    pub playlists: Vec<String>,
}
```

### [LIB-47] Correspondance

Un titre entre dans la catégorie s'il remplit tous les critères renseignés ; sans année ou sans BPM, il ne passe pas un filtre d'année ou de BPM.

`crates/blast-library/src/rotation.rs` ligne 49

```rust
pub fn matches(&self, t: &LibraryTrack) -> bool {
    if !self.kinds.is_empty() && !self.kinds.contains(&t.kind) {
        return false;
    }
    if !self.genres.is_empty() {
        let genre = t.genre.to_lowercase();
        let hit = self
            .genres
            .iter()
            .map(|g| g.trim().to_lowercase())
            .any(|g| !g.is_empty() && genre.contains(&g));
        if !hit {
…
```

### [LIB-48] Catégorie de grille

Nom, couleur et règles.

`crates/blast-library/src/rotation.rs` ligne 102

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RotationCategory {
    pub id: String,
    pub name: String,
    pub color: String,
    pub rule: CategoryRule,
}
```

### [LIB-49] Grille

Nom, couleur et suite ordonnée de catégories, jouée en boucle.

`crates/blast-library/src/rotation.rs` ligne 112

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clock {
    pub id: String,
    pub name: String,
    pub color: String,
    pub slots: Vec<String>,
}
```

### [LIB-50] Grilles et planning

Catégories, grilles, émissions, contenu de chaque heure de la semaine (grille ou émission) et grille hors planning.

`crates/blast-library/src/rotation.rs` ligne 132

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rotation {
    pub categories: Vec<RotationCategory>,
    pub clocks: Vec<Clock>,
    pub programs: Vec<Program>,
    pub schedule: Vec<String>,
    pub default_clock: Option<String>,
}
```

### [LIB-51] Validation des grilles

Noms présents, identifiants uniques, aucune référence à une catégorie ou une grille supprimée.

`crates/blast-library/src/rotation.rs` ligne 186

```rust
pub fn validate(&self) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for c in &self.categories {
        if c.name.trim().is_empty() {
            return Err("Chaque catégorie doit avoir un nom.".into());
        }
        if c.id.is_empty() || !ids.insert(c.id.as_str()) {
            return Err(format!(
                "Identifiant de catégorie invalide : « {} ».",
                c.name
            ));
        }
…
```

### [LIB-52] Contenu d'une heure

Grille ou émission du planning pour un jour et une heure, sinon grille hors planning ; une grille sans case est ignorée.

`crates/blast-library/src/rotation.rs` ligne 237

```rust
pub fn scheduled(rotation: &Rotation, weekday: usize, hour: usize) -> Option<Scheduled<'_>> {
    let slot = weekday.min(6) * 24 + hour.min(23);
    if let Some(id) = rotation.schedule.get(slot).filter(|id| !id.is_empty()) {
        if let Some(p) = rotation.program(id) {
            return Some(Scheduled::Program(p));
        }
        if let Some(c) = rotation.clock(id).filter(|c| !c.slots.is_empty()) {
            return Some(Scheduled::Clock(c));
        }
    }
    default_clock(rotation).map(Scheduled::Clock)
}
```

### [LIB-53] Poids d'un titre

Heures écoulées depuis le dernier passage (une semaine au plus), plus un : un titre oublié pèse beaucoup plus qu'un titre récent.

`crates/blast-library/src/rotation.rs` ligne 260

```rust
pub fn weight(t: &LibraryTrack, now: i64) -> f64 {
    let hours = t
        .last_played
        .map_or(STALE_HOURS, |at| (now - at).max(0) as f64 / 3600.0);
    hours.min(STALE_HOURS) + 1.0
}
```

### [LIB-54] Tirage pondéré

Choisit un titre au hasard proportionnellement à son poids.

`crates/blast-library/src/rotation.rs` ligne 268

```rust
pub fn pick_weighted<'a>(
    candidates: &[&'a LibraryTrack],
    now: i64,
    roll: f64,
) -> Option<&'a LibraryTrack> {
    let total: f64 = candidates.iter().map(|t| weight(t, now)).sum();
    if candidates.is_empty() || total <= 0.0 {
        return None;
    }
    let mut target = roll.clamp(0.0, 0.999_999) * total;
    for t in candidates {
        target -= weight(t, now);
…
```

### [LIB-56] Contenu des playlists

Pour chaque playlist M3U (clé normalisée), l'ensemble des chemins normalisés qu'elle contient.

`crates/blast-library/src/rotation.rs` ligne 26

```rust
pub type PlaylistSets = HashMap<String, HashSet<String>>;
```

### [LIB-57] Clé de chemin

Chemin normalisé pour comparer : barres obliques, minuscules, sans barre finale.

`crates/blast-library/src/rotation.rs` ligne 29

```rust
pub fn path_key(p: &str) -> String {
    norm_path(p)
}
```

### [LIB-58] Correspondance avec playlists

Applique les règles d'une catégorie, puis exige que le titre figure dans l'une de ses playlists M3U s'il y en a.

`crates/blast-library/src/rotation.rs` ligne 86

```rust
pub fn matches_with(&self, t: &LibraryTrack, lists: &PlaylistSets) -> bool {
    if !self.matches(t) {
        return false;
    }
    if self.playlists.is_empty() {
        return true;
    }
    let key = norm_path(&t.path);
    self.playlists
        .iter()
        .filter_map(|p| lists.get(&norm_path(p)))
        .any(|set| set.contains(&key))
…
```

### [LIB-59] Émission

Nom, couleur et fichier M3U d'une émission à placer dans le planning.

`crates/blast-library/src/rotation.rs` ligne 122

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Program {
    pub id: String,
    pub name: String,
    pub color: String,
    pub path: String,
}
```

### [LIB-60] Contenu programmé

Une heure du planning contient une grille ou une émission.

`crates/blast-library/src/rotation.rs` ligne 143

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scheduled<'a> {
    Clock(&'a Clock),
    Program(&'a Program),
}
```

### [LIB-61] Playlists utilisées

Liste sans doublon des fichiers M3U cités par les catégories et les émissions.

`crates/blast-library/src/rotation.rs` ligne 172

```rust
pub fn playlist_paths(&self) -> Vec<String> {
    let mut out: Vec<String> = self
        .categories
        .iter()
        .flat_map(|c| c.rule.playlists.iter().cloned())
        .chain(self.programs.iter().map(|p| p.path.clone()))
        .filter(|p| !p.trim().is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}
```

### [LIB-62] Grille hors planning

Grille utilisée pour les heures vides du planning et après la fin d'une émission ; ignorée si elle n'a aucune case.

`crates/blast-library/src/rotation.rs` ligne 251

```rust
pub fn default_clock(rotation: &Rotation) -> Option<&Clock> {
    rotation
        .default_clock
        .as_deref()
        .and_then(|id| rotation.clock(id))
        .filter(|c| !c.slots.is_empty())
}
```


## LIVE

Chat en direct (blast-live, en préparation)

### [LIVE-01] Plateforme de chat

Twitch, YouTube ou Kick.

`crates/blast-live/src/lib.rs` ligne 8

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    YouTube,
    Kick,
}
```

### [LIVE-02] Message de chat

Identifiant, plateforme, nom affiché et identifiant de connexion, couleur, badges, texte, action (/me), message de BLAST lui-même, heure d'envoi.

`crates/blast-live/src/lib.rs` ligne 17

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatMessage {
    pub id: String,
    pub platform: Platform,
    pub author: String,
    pub login: String,
    pub color: String,
    pub badges: Vec<String>,
    pub text: String,
    pub action: bool,
    pub own: bool,
    pub at_ms: u64,
…
```

### [LIVE-03] Commande de chat

Demande de titre, titre en cours ou déclenchement d'un son.

`crates/blast-live/src/lib.rs` ligne 53

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ChatCommand {
    Request(String),
    NowPlaying,
    Sfx(String),
}
```

### [LIVE-04] Lecture des commandes

Reconnaît !request (ou !sr), !np (ou !song) et !sfx dans un message.

`crates/blast-live/src/lib.rs` ligne 61

```rust
pub fn parse_command(text: &str) -> Option<ChatCommand> {
    let text = text.trim();
    let rest = text.strip_prefix('!')?;
    let (cmd, arg) = match rest.split_once(char::is_whitespace) {
        Some((c, a)) => (c, a.trim()),
        None => (rest, ""),
    };
    match cmd.to_lowercase().as_str() {
        "request" | "sr" if !arg.is_empty() => Some(ChatCommand::Request(arg.to_string())),
        "np" | "song" => Some(ChatCommand::NowPlaying),
        "sfx" if !arg.is_empty() => Some(ChatCommand::Sfx(arg.to_string())),
        _ => None,
…
```

### [LIVE-05] Événement de chat

État de connexion (connecté, écriture possible, détail), message, annonce (abonnement, raid…), effacement des messages d'un viewer ou de tout le chat, suppression d'un message.

`crates/blast-live/src/lib.rs` ligne 32

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEvent {
    Status {
        connected: bool,
        writable: bool,
        detail: String,
    },
    Message(ChatMessage),
    Notice {
        text: String,
    },
…
```

### [LIVE-06] Ligne IRC

Tags, préfixe, commande et paramètres d'une ligne du protocole IRC de Twitch.

`crates/blast-live/src/irc.rs` ligne 3

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IrcMessage {
    pub tags: HashMap<String, String>,
    pub prefix: String,
    pub command: String,
    pub params: Vec<String>,
}
```

### [LIVE-07] Valeur de tag

Décode les caractères échappés d'une valeur de tag IRC (\s pour l'espace, \: pour le point-virgule…).

`crates/blast-live/src/irc.rs` ligne 26

```rust
fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(':') => out.push(';'),
            Some('s') => out.push(' '),
            Some('\\') => out.push('\\'),
…
```

### [LIVE-08] Lecture d'une ligne IRC

Découpe tags, préfixe, commande, paramètres et texte final.

`crates/blast-live/src/irc.rs` ligne 48

```rust
pub fn parse_line(line: &str) -> Option<IrcMessage> {
    let mut rest = line.trim_end_matches(['\r', '\n']);
    if rest.is_empty() {
        return None;
    }
    let mut msg = IrcMessage::default();
    if let Some(r) = rest.strip_prefix('@') {
        let (tags, after) = r.split_once(' ')?;
        for pair in tags.split(';') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            msg.tags.insert(k.to_string(), unescape(v));
        }
…
```

### [LIVE-09] Rythmes du client Twitch

Pause de la boucle, délai minimal entre deux messages envoyés (20 par 30 s), silence maximal avant reconnexion, attente maximale entre deux essais.

`crates/blast-live/src/twitch.rs` ligne 12

```rust
const IDLE: Duration = Duration::from_millis(15);
```

### [LIVE-10] Réglages de connexion Twitch

Chaîne, compte et jeton (facultatifs), serveur, port et chiffrement ; l'écriture n'est possible qu'avec un compte et un jeton.

`crates/blast-live/src/twitch.rs` ligne 18

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TwitchConfig {
    pub channel: String,
    pub login: String,
    pub token: String,
    pub host: String,
    pub port: u16,
    pub tls: bool,
}
```

### [LIVE-11] Nom de chaîne

Accepte un nom, #nom ou un lien twitch.tv/… et renvoie le nom en minuscules.

`crates/blast-live/src/twitch.rs` ligne 46

```rust
pub fn normalize_channel(input: &str) -> String {
    let s = input.trim();
    let s = s
        .rsplit_once("twitch.tv/")
        .map(|(_, rest)| rest)
        .unwrap_or(s);
    s.trim_start_matches('#')
        .split(['/', '?', ' '])
        .next()
        .unwrap_or("")
        .to_lowercase()
}
```

### [LIVE-12] Transport

Connexion TCP non bloquante, chiffrée par rustls (certificat vérifié par le magasin du système) ou en clair pour les tests.

`crates/blast-live/src/twitch.rs` ligne 67

```rust
enum Transport {
    Plain(TcpStream),
    Tls(Box<rustls::ClientConnection>, TcpStream),
}
```

### [LIVE-13] Fin de session

Arrêt demandé, reconnexion demandée par Twitch, connexion perdue ou refus définitif (jeton refusé).

`crates/blast-live/src/twitch.rs` ligne 192

```rust
enum End {
    Stopped,
    Reconnect,
    Lost(String),
    Fatal(String),
}
```

### [LIVE-14] Badges

Noms des badges d'un message (broadcaster, moderator, vip, subscriber…).

`crates/blast-live/src/twitch.rs` ligne 207

```rust
fn badges(tag: &str) -> Vec<String> {
    tag.split(',')
        .filter_map(|b| b.split('/').next())
        .filter(|b| !b.is_empty())
        .map(str::to_string)
        .collect()
}
```

### [LIVE-15] Message Twitch

Convertit un PRIVMSG en message de chat, avec le nom affiché, la couleur, les badges et les actions /me.

`crates/blast-live/src/twitch.rs` ligne 216

```rust
fn to_message(m: &IrcMessage) -> ChatMessage {
    let raw = m.trailing();
    let (text, action) = match raw
        .strip_prefix("\u{1}ACTION ")
        .and_then(|t| t.strip_suffix('\u{1}'))
    {
        Some(t) => (t.to_string(), true),
        None => (raw.to_string(), false),
    };
    let login = m.nick().to_string();
    let display = m.tag("display-name");
    ChatMessage {
…
```

### [LIVE-16] Session IRC

Identification (anonyme justinfan ou compte + jeton), rejoint la chaîne, répond aux PING, traduit les messages, envoie la file d'attente au rythme autorisé et renvoie localement les messages envoyés.

`crates/blast-live/src/twitch.rs` ligne 246

```rust
fn session(
    cfg: &TwitchConfig,
    emit: &(dyn Fn(ChatEvent) + Send),
    out: &Receiver<String>,
    stop: &AtomicBool,
) -> End {
    let mut t = match Transport::open(cfg) {
        Ok(t) => t,
        Err(e) => return End::Lost(format!("connexion impossible : {e}")),
    };
    let writable = cfg.writable();
    let nick = if writable {
…
```

### [LIVE-17] Boucle de connexion

Enchaîne les sessions avec une attente croissante (1 à 30 s) ; s'arrête sur un jeton refusé.

`crates/blast-live/src/twitch.rs` ligne 399

```rust
fn run(
    cfg: TwitchConfig,
    emit: Box<dyn Fn(ChatEvent) + Send>,
    out: Receiver<String>,
    stop: Arc<AtomicBool>,
) {
    let writable = cfg.writable();
    let mut backoff = 1;
    while !stop.load(Ordering::Relaxed) {
        emit(ChatEvent::Status {
            connected: false,
            writable,
…
```

### [LIVE-18] Client Twitch

Fil de connexion, file des messages à envoyer et arrêt propre.

`crates/blast-live/src/twitch.rs` ligne 450

```rust
pub struct TwitchClient {
    out: Sender<String>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    pub config: TwitchConfig,
}
```


## ICE

Serveur Icecast : configuration icecast.xml, statistiques, administration et processus (blast-ice)

### [ICE-01] Erreurs Icecast

Serveur injoignable, identifiants admin refusés, statut HTTP inattendu, réponse illisible, action refusée par Icecast ou erreur de fichier, avec un message en français.

`crates/blast-ice/src/lib.rs` ligne 13

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IceError {
    Unreachable(String),
    Unauthorized,
    Http(u16),
    Invalid(String),
    Refused(String),
    Io(String),
}
```

### [ICE-02] Mot de passe aléatoire

Caractères alphanumériques sans les signes ambigus (0, O, 1, l, I), tirés par le générateur aléatoire du système.

`crates/blast-ice/src/lib.rs` ligne 46

```rust
pub fn random_password(len: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = vec![0u8; len];
    if getrandom::fill(&mut bytes).is_err() {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (seed >> ((i % 16) * 8)) as u8 ^ (i as u8).wrapping_mul(97);
        }
    }
…
```

### [ICE-03] Limites du serveur

Clients, sources, taille de file par auditeur, délais client, en-têtes et source, rafale à la connexion ; valeurs par défaut d'Icecast.

`crates/blast-ice/src/config.rs` ligne 7

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerLimits {
    pub clients: u32,
    pub sources: u32,
    pub queue_size: u32,
    pub client_timeout: u32,
    pub header_timeout: u32,
    pub source_timeout: u32,
    pub burst_size: u32,
}
```

### [ICE-04] Authentification du serveur

Mots de passe des sources et des relais, identifiant et mot de passe admin.

`crates/blast-ice/src/config.rs` ligne 34

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerAuth {
    pub source_password: String,
    pub relay_password: String,
    pub admin_user: String,
    pub admin_password: String,
}
```

### [ICE-05] Port d'écoute

Port, adresse d'écoute, compatibilité Shoutcast (qui occupe aussi le port suivant) et HTTPS (<ssl> en 2.4, <tls> en 2.5). `key` désigne la balise d'origine pour en garder les options non gérées.

`crates/blast-ice/src/config.rs` ligne 44

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ListenSocket {
    pub key: Option<usize>,
    pub port: u16,
    pub bind_address: String,
    pub shoutcast_mount: String,
    pub ssl: bool,
}
```

### [ICE-06] Mountpoint déclaré

Nom, identifiant et mot de passe propres, limite d'auditeurs, secours et retour automatique, présentation imposée, annuaire (laisser la source décider, toujours, jamais) et masquage.

`crates/blast-ice/src/config.rs` ligne 55

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mount {
    pub key: Option<usize>,
    pub mount_name: String,
    pub username: String,
    pub password: String,
    pub max_listeners: u32,
    pub fallback_mount: String,
    pub fallback_override: bool,
    pub public: Option<bool>,
    pub stream_name: String,
…
```

### [ICE-07] Chemins

basedir, logdir, webroot, adminroot, fichier PID et certificat TLS (paths/ssl-certificate en 2.4, security/tls-context/tls-certificate en 2.5).

`crates/blast-ice/src/config.rs` ligne 75

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Paths {
    pub basedir: String,
    pub logdir: String,
    pub webroot: String,
    pub adminroot: String,
    pub pidfile: String,
    pub ssl_certificate: String,
}
```

### [ICE-08] Journalisation

Fichiers d'accès et d'erreurs, niveau de 1 à 4, taille maximale et archivage (balise ajoutée seulement si l'archivage est activé).

`crates/blast-ice/src/config.rs` ligne 87

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Logging {
    pub accesslog: String,
    pub errorlog: String,
    pub loglevel: u8,
    pub logsize: u32,
    pub logarchive: bool,
}
```

### [ICE-09] Configuration éditable

Partie de icecast.xml présentée dans l'interface : serveur, sécurité, ports, limites, mountpoints, chemins et journaux.

`crates/blast-ice/src/config.rs` ligne 110

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IceConfig {
    pub location: String,
    pub admin: String,
    pub hostname: String,
    pub limits: ServerLimits,
    pub auth: ServerAuth,
    pub sockets: Vec<ListenSocket>,
    pub mounts: Vec<Mount>,
    pub paths: Paths,
    pub logging: Logging,
…
```

### [ICE-10] Modèle de configuration neuve

icecast.xml minimal : un port 8000, en-tête CORS, alias de la page d'état, chroot désactivé.

`crates/blast-ice/src/config.rs` ligne 125

```rust
const STARTER: &str = r#"<icecast>
    <location>Terre</location>
    <admin>icemaster@localhost</admin>
    <hostname>localhost</hostname>
    <limits></limits>
    <authentication></authentication>
    <listen-socket><port>8000</port></listen-socket>
    <http-headers>
        <header name="Access-Control-Allow-Origin" value="*" />
    </http-headers>
    <fileserve>1</fileserve>
    <paths>
…
```

### [ICE-11] Document icecast.xml

Arbre XML complet du fichier. Seules les balises gérées sont modifiées : commentaires, relais, authentification des mountpoints, en-têtes et sécurité sont conservés.

`crates/blast-ice/src/config.rs` ligne 149

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct IceDocument {
    root: Element,
}
```

### [ICE-12] Lecture de la configuration

Extrait la configuration éditable ; une balise absente prend la valeur par défaut d'Icecast, une limite d'auditeurs négative signifie « sans limite ».

`crates/blast-ice/src/config.rs` ligne 296

```rust
pub fn read(&self) -> IceConfig {
    let r = &self.root;
    let empty = Element::new("");
    let limits = r.get_child("limits").unwrap_or(&empty);
    let auth = r.get_child("authentication").unwrap_or(&empty);
    let paths = r.get_child("paths").unwrap_or(&empty);
    let logging = r.get_child("logging").unwrap_or(&empty);
    let d = ServerLimits::default();
    let ld = Logging::default();
    IceConfig {
        location: text(r, "location"),
        admin: text(r, "admin"),
…
```

### [ICE-13] Application de la configuration

Réécrit les balises gérées ; une valeur vide retire la balise. Ports et mountpoints sont reconstruits à l'emplacement des anciens, à partir de leur balise d'origine.

`crates/blast-ice/src/config.rs` ligne 386

```rust
pub fn apply(&mut self, cfg: &IceConfig) {
    let modern = modern(&self.root);
    let r = &mut self.root;
    set(r, "location", &cfg.location);
    set(r, "admin", &cfg.admin);
    set(r, "hostname", &cfg.hostname);

    let l = child_mut(r, "limits");
    let lim = &cfg.limits;
    for (name, v) in [
        ("clients", lim.clients),
        ("sources", lim.sources),
…
```

### [ICE-14] Écriture du XML

Indentation de quatre espaces, sans déclaration XML, comme le fichier fourni avec Icecast.

`crates/blast-ice/src/config.rs` ligne 529

```rust
pub fn to_xml(&self) -> String {
    let mut out = Vec::new();
    let config = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("    ")
        .write_document_declaration(false);
    if self.root.write_with_config(&mut out, config).is_err() {
        return String::new();
    }
    let mut text = String::from_utf8(out).unwrap_or_default();
    text.push('\n');
    text
…
```

### [ICE-15] Validation

Refuse un hostname vide, l'absence de port, un port en double (compatibilité Shoutcast comprise), un mot de passe vide, des limites nulles, un niveau de journal hors de 1 à 4, un mountpoint mal nommé ou en double, un secours qui pointe sur lui-même. Avertit pour un mot de passe « hackme » ou un secours non déclaré.

`crates/blast-ice/src/config.rs` ligne 545

```rust
impl IceConfig {
    pub fn validate(&self) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        if self.hostname.trim().is_empty() {
            return Err("Indique le nom d'hôte du serveur (hostname).".into());
        }
        if self.sockets.is_empty() {
            return Err("Il faut au moins un port d'écoute.".into());
        }
        let mut taken: Vec<(u16, &str)> = Vec::new();
        for s in &self.sockets {
            if s.port == 0 {
…
```

### [ICE-16] Chemins absolus

Rend webroot, adminroot, basedir et le certificat absolus par rapport au dossier d'installation, et place journaux et fichier PID dans le dossier indiqué. Sert quand la configuration est copiée hors du dossier d'Icecast.

`crates/blast-ice/src/config.rs` ligne 646

```rust
pub fn rebase(&mut self, base: &Path, log_dir: &Path) {
    let absolute = |p: &str| -> String {
        let trimmed = p.trim();
        if trimmed.is_empty() || Path::new(trimmed).is_absolute() {
            return trimmed.to_string();
        }
        let rel = trimmed.trim_start_matches("./").trim_start_matches(".\\");
        base.join(rel).display().to_string()
    };
    self.paths.basedir = absolute(&self.paths.basedir);
    self.paths.webroot = absolute(&self.paths.webroot);
    self.paths.adminroot = absolute(&self.paths.adminroot);
…
```

### [ICE-17] Résolution des chemins

Chemin relatif au dossier de travail (ou à basedir), chemin complet d'un journal et premier port d'écoute.

`crates/blast-ice/src/config.rs` ligne 670

```rust
pub fn resolve(&self, workdir: &Path, value: &str) -> PathBuf {
    let p = Path::new(value.trim());
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let base = if self.paths.basedir.trim().is_empty() {
        workdir.to_path_buf()
    } else {
        workdir.join(self.paths.basedir.trim())
    };
    base.join(value.trim().trim_start_matches("./"))
}
```

### [ICE-18] Test d'écriture

Ouvre le fichier en ajout sans le modifier, ou crée un fichier témoin dans le dossier s'il n'existe pas encore.

`crates/blast-ice/src/config.rs` ligne 701

```rust
pub fn is_writable(path: &Path) -> bool {
    if path.exists() {
        return std::fs::OpenOptions::new().append(true).open(path).is_ok();
    }
    let Some(dir) = path.parent() else {
        return false;
    };
    let probe = dir.join(format!(".blast-probe-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}
```

### [ICE-19] Écriture avec sauvegarde

Copie l'ancien fichier en .bak-<horodatage>, écrit un fichier temporaire puis le renomme, et ne garde que les dernières sauvegardes.

`crates/blast-ice/src/config.rs` ligne 715

```rust
pub fn write_file(path: &Path, xml: &str, keep: usize) -> Result<Option<PathBuf>, String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or("chemin de configuration invalide")?;
    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
    if !dir.as_os_str().is_empty() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let mut backup = None;
    if path.exists() {
        let secs = std::time::SystemTime::now()
…
```

### [ICE-20] Liste des sauvegardes

Sauvegardes d'un fichier de configuration, de la plus ancienne à la plus récente.

`crates/blast-ice/src/config.rs` ligne 745

```rust
pub fn backups(path: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Vec::new();
    };
    let prefix = format!("{}.bak-", name.to_string_lossy());
    let mut list: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with(&prefix))
…
```

### [ICE-21] Statistiques d'un mountpoint

Auditeurs, pic, limite, débit, nom, genre, titre en cours (artiste ajouté s'il est séparé), type, adresse et logiciel de la source, début de diffusion, octets envoyés.

`crates/blast-ice/src/stats.rs` ligne 6

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MountStats {
    pub mount: String,
    pub listeners: u32,
    pub listener_peak: u32,
    pub max_listeners: Option<u32>,
    pub slow_listeners: u32,
    pub bitrate: String,
    pub server_name: String,
    pub server_description: String,
    pub genre: String,
    pub title: String,
…
```

### [ICE-22] Statistiques du serveur

Version, hôte, date de lancement, auditeurs, sources, clients, compteurs de connexions et mountpoints actifs.

`crates/blast-ice/src/stats.rs` ligne 30

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ServerStats {
    pub server_id: String,
    pub host: String,
    pub location: String,
    pub admin: String,
    pub server_start: String,
    pub started_at: Option<i64>,
    pub listeners: u32,
    pub sources: u32,
    pub clients: u32,
    pub client_connections: u64,
…
```

### [ICE-23] Auditeur

Identifiant Icecast, adresse IP, lecteur utilisé et durée de connexion en secondes.

`crates/blast-ice/src/stats.rs` ligne 49

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Listener {
    pub id: u64,
    pub ip: String,
    pub user_agent: String,
    pub connected_secs: u64,
}
```

### [ICE-24] Lecture des dates

Date ISO 8601 d'Icecast (décalage +0100 ou +01:00), sinon format RFC 2822 ; renvoie un horodatage Unix.

`crates/blast-ice/src/stats.rs` ligne 76

```rust
pub fn parse_time(iso: &str, rfc: &str) -> Option<i64> {
    let iso = iso.trim();
    if !iso.is_empty() {
        for f in ["%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%dT%H:%M:%S%:z"] {
            if let Ok(t) = chrono::DateTime::parse_from_str(iso, f) {
                return Some(t.timestamp());
            }
        }
    }
    chrono::DateTime::parse_from_rfc2822(rfc.trim())
        .ok()
        .map(|t| t.timestamp())
…
```

### [ICE-25] Lecture de /admin/stats

Transforme la réponse XML des statistiques admin en statistiques du serveur.

`crates/blast-ice/src/stats.rs` ligne 102

```rust
pub fn parse_stats(xml: &str) -> Result<ServerStats, IceError> {
    let r = parse_root(xml)?;
    let mounts = children(&r, "source")
        .map(|s| {
            let title = text(s, "title");
            let artist = text(s, "artist");
            MountStats {
                mount: s.attributes.get("mount").cloned().unwrap_or_default(),
                listeners: num(s, "listeners"),
                listener_peak: num(s, "listener_peak"),
                max_listeners: text(s, "max_listeners").parse().ok(),
                slow_listeners: num(s, "slow_listeners"),
…
```

### [ICE-26] Lecture de /admin/listclients

Transforme la liste XML des auditeurs d'un mountpoint.

`crates/blast-ice/src/stats.rs` ligne 164

```rust
pub fn parse_listeners(xml: &str) -> Result<Vec<Listener>, IceError> {
    let r = parse_root(xml)?;
    Ok(children(&r, "source")
        .flat_map(|s| children(s, "listener"))
        .map(|l| Listener {
            id: {
                let id: u64 = num(l, "ID");
                if id == 0 {
                    l.attributes
                        .get("id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0)
…
```

### [ICE-27] Réponse d'une action admin

Lit <iceresponse> : return vaut 1 en cas de succès, sinon le message est renvoyé comme refus.

`crates/blast-ice/src/stats.rs` ligne 188

```rust
pub fn parse_response(xml: &str) -> Result<String, IceError> {
    let r = Element::parse(xml.as_bytes()).map_err(|e| IceError::Invalid(e.to_string()))?;
    let message = text(&r, "message");
    if text(&r, "return") == "1" {
        Ok(message)
    } else {
        Err(IceError::Refused(if message.is_empty() {
            "sans précision".into()
        } else {
            message
        }))
    }
…
```

### [ICE-28] Actions admin

Déconnecter un auditeur, couper une source, déplacer les auditeurs vers un autre mountpoint, changer le titre affiché.

`crates/blast-ice/src/client.rs` ligne 9

```rust
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AdminAction {
    KillClient { mount: String, id: u64 },
    KillSource { mount: String },
    MoveClients { mount: String, destination: String },
    Metadata { mount: String, song: String },
}
```

### [ICE-29] Point d'accès admin

Hôte, port, HTTPS, identifiant et mot de passe admin, logiciel annoncé. Une adresse IPv6 est mise entre crochets.

`crates/blast-ice/src/client.rs` ligne 19

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub https: bool,
    pub user: String,
    pub password: String,
    pub user_agent: String,
}
```

### [ICE-30] Encodage d'URL

Encode un paramètre de requête en pourcentages, « / » compris dans les caractères gardés.

`crates/blast-ice/src/client.rs` ligne 30

```rust
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
```

### [ICE-31] Requête HTTP

GET avec authentification Basic, délai de 5 s, certificats du système, 401 ou 403 : identifiants refusés, 400 : message de la page d'erreur d'Icecast, corps limité à 8 Mo.

`crates/blast-ice/src/client.rs` ligne 92

```rust
fn get(&self, path: &str, admin: bool) -> Result<String, IceError> {
    let url = format!("{}{path}", self.base());
    let mut req = Self::agent(Duration::from_secs(5))
        .get(&url)
        .header("User-Agent", &self.user_agent);
    if admin {
        let auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.user, self.password));
        req = req.header("Authorization", &format!("Basic {auth}"));
    }
    let mut resp = req
        .call()
…
```

### [ICE-32] Sonde

Interroge /status-json.xsl sans authentification pour savoir si le serveur répond et lire sa version.

`crates/blast-ice/src/client.rs` ligne 124

```rust
pub fn probe(&self) -> Result<String, IceError> {
    let body = self.get("/status-json.xsl", false)?;
    let id = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["icestats"]["server_id"].as_str().map(String::from));
    Ok(id.unwrap_or_else(|| "Icecast".into()))
}
```

### [ICE-33] Statistiques admin

Lit /admin/stats avec les identifiants admin.

`crates/blast-ice/src/client.rs` ligne 133

```rust
pub fn stats(&self) -> Result<ServerStats, IceError> {
    parse_stats(&self.get("/admin/stats", true)?)
}
```

### [ICE-34] Liste des auditeurs

Lit /admin/listclients pour un mountpoint.

`crates/blast-ice/src/client.rs` ligne 138

```rust
pub fn listeners(&self, mount: &str) -> Result<Vec<Listener>, IceError> {
    parse_listeners(&self.get(&format!("/admin/listclients?mount={}", encode(mount)), true)?)
}
```

### [ICE-35] Exécution d'une action admin

Appelle killclient, killsource, moveclients ou metadata (mode updinfo) et renvoie le message d'Icecast.

`crates/blast-ice/src/client.rs` ligne 143

```rust
pub fn run(&self, action: &AdminAction) -> Result<String, IceError> {
    let path = match action {
        AdminAction::KillClient { mount, id } => {
            format!("/admin/killclient?mount={}&id={id}", encode(mount))
        }
        AdminAction::KillSource { mount } => {
            format!("/admin/killsource?mount={}", encode(mount))
        }
        AdminAction::MoveClients { mount, destination } => format!(
            "/admin/moveclients?mount={}&destination={}",
            encode(mount),
            encode(destination)
…
```

### [ICE-36] Installation détectée

Dossier, exécutable, fichier de configuration, dossiers web et admin d'une installation d'Icecast.

`crates/blast-ice/src/process.rs` ligne 8

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Install {
    pub dir: PathBuf,
    pub exe: PathBuf,
    pub config: Option<PathBuf>,
    pub web: Option<PathBuf>,
    pub admin: Option<PathBuf>,
}
```

### [ICE-37] Recherche dans un dossier

Cherche icecast.exe, icecast2.exe, icecast ou icecast2 à la racine ou dans bin, puis icecast.xml et les dossiers web et admin.

`crates/blast-ice/src/process.rs` ligne 26

```rust
pub fn find_in(dir: &Path) -> Option<Install> {
    let names = ["icecast.exe", "icecast2.exe", "icecast", "icecast2"];
    let exes: Vec<PathBuf> = names
        .iter()
        .flat_map(|n| [dir.join("bin").join(n), dir.join(n)])
        .collect();
    let exe = first_file(&exes)?;
    Some(Install {
        dir: dir.to_path_buf(),
        exe,
        config: first_file(&[
            dir.join("icecast.xml"),
…
```

### [ICE-38] Détection

Sous Windows, les dossiers de Program Files dont le nom contient « icecast » ; ailleurs, les paquets icecast2, icecast, /usr/local et Homebrew.

`crates/blast-ice/src/process.rs` ligne 53

```rust
pub fn detect() -> Vec<Install> {
    let mut found = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            let Some(root) = std::env::var_os(var).map(PathBuf::from) else {
                continue;
            };
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
…
```

### [ICE-39] Processus Icecast

Lance l'exécutable avec -c dans son dossier de travail, sortie vers console.log, sans fenêtre de console sous Windows ; échec si Icecast s'arrête dans les 1,2 s. Reprend un PID existant. Arrêt par SIGTERM puis arrêt forcé, ou TerminateProcess sous Windows.

`crates/blast-ice/src/process.rs` ligne 115

```rust
pub struct Process {
    child: Option<Child>,
    pub pid: u32,
}
```

### [ICE-40] Arrêt par PID

taskkill /F sous Windows, kill -KILL ailleurs, pour un Icecast lancé lors d'une session précédente de BLAST.

`crates/blast-ice/src/process.rs` ligne 211

```rust
pub fn stop_pid(pid: u32) -> Result<(), String> {
    let status = if cfg!(windows) {
        quiet(&mut Command::new("taskkill"))
            .args(["/PID", &pid.to_string(), "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        quiet(&mut Command::new("kill"))
            .args(["-KILL", &pid.to_string()])
            .status()
    };
…
```

### [ICE-41] Vérification d'un PID

Le nom du processus contient « icecast » (/proc sous Linux, tasklist sous Windows, ps sinon), pour ne jamais arrêter un autre programme qui aurait repris le même numéro.

`crates/blast-ice/src/process.rs` ligne 231

```rust
pub fn pid_is_icecast(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let name = if cfg!(target_os = "linux") {
        std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default()
    } else if cfg!(windows) {
        quiet(&mut Command::new("tasklist"))
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
…
```

### [ICE-42] Fin d'un fichier

Dernières lignes d'un journal, en lisant au plus les 256 derniers Ko.

`crates/blast-ice/src/process.rs` ligne 254

```rust
pub fn tail(path: &Path, lines: usize) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let window = 256 * 1024;
    let start = len.saturating_sub(window);
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    let all: Vec<&str> = text.lines().collect();
    let skip = usize::from(start > 0);
    let from = all.len().saturating_sub(lines).max(skip.min(all.len()));
…
```

### [ICE-43] Message d'une page d'erreur

Texte d'une page d'erreur HTML d'Icecast, sans le code HTTP placé devant.

`crates/blast-ice/src/client.rs` ligne 44

```rust
pub fn error_message(body: &str) -> Option<String> {
    let inner = body.split_once("<body>").map(|(_, b)| b).unwrap_or(body);
    let mut text = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
…
```

### [ICE-44] Format Icecast 2.5

Reconnaît un fichier au format 2.5 : bloc security/tls-context, balise <tls> dans un port ou niveau de journal écrit en toutes lettres. La réécriture garde alors ce format.

`crates/blast-ice/src/config.rs` ligne 201

```rust
fn modern(r: &Element) -> bool {
    let tls_context = r
        .get_child("security")
        .is_some_and(|s| s.get_child("tls-context").is_some());
    let tls_socket = elements(r, "listen-socket").any(|s| s.get_child("tls").is_some());
    let level = r
        .get_child("logging")
        .map(|l| text(l, "loglevel"))
        .unwrap_or_default();
    tls_context || tls_socket || (!level.is_empty() && level.parse::<u8>().is_err())
}
```

### [ICE-45] Niveau de journal

Lit le niveau en chiffre (1 à 4, Icecast 2.4) ou en toutes lettres (error, warning, information, debug, Icecast 2.5) ; il est réécrit dans la forme du fichier.

`crates/blast-ice/src/config.rs` ligne 214

```rust
fn level_from(value: &str, default: u8) -> u8 {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" | "eror" => 1,
        "warning" | "warn" => 2,
        "information" | "info" => 3,
        "debug" | "dbug" => 4,
        v => v.parse().unwrap_or(default),
    }
}
```


## CLI

Outil en ligne de commande « blast » (blast-cli)

### [CLI-01] Arguments

Point d'entrée des arguments de la commande blast.

`crates/blast-cli/src/main.rs` ligne 17

```rust
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}
```

### [CLI-02] Sous-commandes

devices, info, play et render.

`crates/blast-cli/src/main.rs` ligne 23

```rust
#[derive(Subcommand)]
enum Cmd {
    #[command(about = "Liste les sorties audio disponibles")]
    Devices,
    #[command(about = "Affiche les informations des fichiers (durée, fréquence, tags)")]
    Info {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    #[command(
        about = "Joue une liste de fichiers en les enchaînant par un fondu sur les platines A et B"
    )]
…
```

### [CLI-03] Lancement

Analyse les arguments et exécute la sous-commande.

`crates/blast-cli/src/main.rs` ligne 68

```rust
fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::Devices => devices(),
        Cmd::Info { files } => info(&files),
        Cmd::Play {
            files,
            fade,
            device,
        } => play(&files, fade, device.as_deref()),
        Cmd::Render {
            files,
…
```

### [CLI-04] Résultat générique

Résultat avec erreur quelconque pour la ligne de commande.

`crates/blast-cli/src/main.rs` ligne 96

```rust
type AnyResult = Result<(), Box<dyn std::error::Error>>;
```

### [CLI-05] Horodatage

Formate une durée en minutes et secondes.

`crates/blast-cli/src/main.rs` ligne 99

```rust
fn clock(frames: usize, rate: u32) -> String {
    let secs = frames as u64 / rate.max(1) as u64;
    format!("{:02}:{:02}", secs / 60, secs % 60)
}
```

### [CLI-06] blast devices

Liste les sorties et entrées audio.

`crates/blast-cli/src/main.rs` ligne 105

```rust
fn devices() -> AnyResult {
    let default = engine::default_output_device();
    for name in engine::list_output_devices()? {
        let mark = if Some(&name) == default.as_ref() {
            "*"
        } else {
            " "
        };
        println!("{mark} {name}");
    }
    Ok(())
}
```

### [CLI-07] blast info

Affiche les tags et la durée de fichiers audio.

`crates/blast-cli/src/main.rs` ligne 119

```rust
fn info(files: &[PathBuf]) -> AnyResult {
    for path in files {
        match decode_file(path, 48_000) {
            Ok(t) => println!(
                "{}  {}  {} Hz  {}",
                clock(t.frames(), t.sample_rate),
                path.display(),
                t.source_rate,
                t.display_name()
            ),
            Err(e) => println!("--:--  {}  {e}", path.display()),
        }
…
```

### [CLI-08] Enchaîneur

Charge les fichiers à tour de rôle sur les deux platines.

`crates/blast-cli/src/main.rs` ligne 136

```rust
struct Sequencer {
    files: Vec<PathBuf>,
    next: usize,
    finished: usize,
    rate: u32,
    names: [String; 2],
}
```

### [CLI-09] Logique d'enchaînement

Choisit la platine libre et le fichier suivant à chaque fin de titre.

`crates/blast-cli/src/main.rs` ligne 145

```rust
impl Sequencer {
    fn new(files: &[PathBuf], rate: u32) -> Self {
        Self {
            files: files.to_vec(),
            next: 0,
            finished: 0,
            rate,
            names: [String::new(), String::new()],
        }
    }

    fn load_next(&mut self, deck: DeckId) -> Option<Arc<Track>> {
…
```

### [CLI-10] Ligne de platine

Affiche l'état d'une platine dans le terminal.

`crates/blast-cli/src/main.rs` ligne 205

```rust
fn deck_line(id: DeckId, s: &DeckStatus, rate: u32, name: &str) -> String {
    let icon = match s.state {
        DeckState::Playing => "▶",
        DeckState::Paused => "‖",
        DeckState::Loaded => "■",
        DeckState::Finished => "✓",
        DeckState::Empty => "·",
    };
    let short: String = name.chars().take(28).collect();
    format!(
        "[{id}] {icon} {}/{} {:<28}",
        clock(s.position, rate),
…
```

### [CLI-11] Crossfader texte

Dessine la position du crossfader.

`crates/blast-cli/src/main.rs` ligne 223

```rust
fn fader(pos: f32) -> String {
    let width = 11;
    let cursor = (pos.clamp(0.0, 1.0) * (width - 1) as f32).round() as usize;
    (0..width)
        .map(|i| if i == cursor { '●' } else { '━' })
        .collect()
}
```

### [CLI-12] Vumètre texte

Dessine un vumètre en caractères.

`crates/blast-cli/src/main.rs` ligne 232

```rust
fn meter(peak: f32) -> String {
    let width = 10;
    let filled = (peak.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("{}{}", "█".repeat(filled), " ".repeat(width - filled))
}
```

### [CLI-13] blast play

Joue des fichiers en auto-mix sur la sortie audio.

`crates/blast-cli/src/main.rs` ligne 239

```rust
fn play(files: &[PathBuf], fade: f32, device: Option<&str>) -> AnyResult {
    let engine = Engine::start(device)?;
    let rate = engine.sample_rate();
    println!(
        "Sortie : {} ({} Hz, {} canaux) — fondu {:.1} s — Ctrl+C pour arrêter",
        engine.device_name(),
        rate,
        engine.channels(),
        fade
    );

    let mut seq = Sequencer::new(files, rate);
…
```

### [CLI-14] blast render

Mixe des fichiers hors temps réel vers un fichier WAV.

`crates/blast-cli/src/main.rs` ligne 295

```rust
fn render(files: &[PathBuf], output: &Path, fade: f32, rate: u32, pcm16: bool) -> AnyResult {
    let mut mixer = Mixer::new(rate);
    let mut seq = Sequencer::new(files, rate);
    let mut pending: Vec<Command> = Vec::new();
    seq.start(|c| pending.push(c), fade);
    for c in pending.drain(..) {
        mixer.apply(c);
    }

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
…
```


## APP

Démarrage de l'application et commandes Tauri

### [APP-01] Démarrage

Ouvre le journal, charge les réglages, lance le contrôleur et ajoute les fichiers passés en argument.

`app/src-tauri/src/main.rs` ligne 18

```rust
fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let paths = app.path();
    let log_path = paths
        .app_log_dir()
        .map(|d| d.join("blast.log"))
        .unwrap_or_else(|_| PathBuf::from("blast.log"));
    if let Err(e) = journal::init(&log_path) {
        eprintln!("journal {} : {e}", log_path.display());
    }
    let config_path = paths
        .app_config_dir()
        .map(|d| d.join("config.toml"))
…
```

### [APP-02] Point d'entrée

Construit l'application Tauri, déclare les commandes et journalise un arrêt anormal.

`app/src-tauri/src/main.rs` ligne 79

```rust
fn main() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            api::blast,
            api::themes,
            api::themes_dir,
            api::get_settings,
            api::save_output,
            api::delete_output,
            api::test_output,
…
```

### [APP-03] État partagé

Canal vers le contrôleur, réglages en mémoire, chemins et trousseau.

`app/src-tauri/src/api.rs` ligne 19

```rust
pub struct App {
    pub tx: Sender<UiCommand>,
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub secrets: Arc<Secrets>,
    pub ice: crate::icecast::IceManager,
}
```

### [APP-04] Sortie vue par l'interface

Réglages d'une sortie et présence d'un mot de passe enregistré.

`app/src-tauri/src/api.rs` ligne 29

```rust
#[derive(Serialize)]
pub struct OutputView {
    #[serde(flatten)]
    cfg: OutputConfig,
    has_password: bool,
}
```

### [APP-05] Réglages vus par l'interface

Tout ce qu'affiche la fenêtre Réglages : sorties, OBS, audio, voix, pads, automation et périphériques.

`app/src-tauri/src/api.rs` ligne 37

```rust
#[derive(Serialize)]
pub struct SettingsView {
    outputs: Vec<OutputView>,
    aura: AuraSettings,
    aura_has_password: bool,
    chat: crate::config::ChatSettings,
    chat_has_token: bool,
    audio: AudioSettings,
    automation: AutomationSettings,
    categories: CategorySettings,
    jingles: JingleSettings,
    library_dirs: Vec<PathBuf>,
…
```

### [APP-06] Écriture des réglages

Construit la vue des réglages, enregistre une modification puis la transmet au contrôleur, range un mot de passe.

`app/src-tauri/src/api.rs` ligne 66

```rust
impl App {
    fn view(&self, warning: Option<String>) -> SettingsView {
        let cfg = self.config.lock().map(|c| c.clone()).unwrap_or_default();
        SettingsView {
            outputs: cfg
                .outputs
                .iter()
                .map(|o| OutputView {
                    has_password: self.secrets.has(&output_key(&o.id)),
                    cfg: o.clone(),
                })
                .collect(),
…
```

### [APP-07] Commande blast

Transmet une commande de l'interface au contrôleur.

`app/src-tauri/src/api.rs` ligne 126

```rust
#[tauri::command]
pub fn blast(state: tauri::State<'_, App>, cmd: UiCommand) -> Result<(), String> {
    state.tx.send(cmd).map_err(|e| e.to_string())
}
```

### [APP-08] Commande themes

Liste les thèmes intégrés et ceux de l'utilisateur.

`app/src-tauri/src/api.rs` ligne 132

```rust
#[tauri::command]
pub fn themes(app: tauri::AppHandle) -> Vec<serde_json::Value> {
    let dir = app.path().app_config_dir().ok().map(|d| d.join("themes"));
    themes::list(dir.as_deref())
}
```

### [APP-09] Commande themes_dir

Crée et renvoie le dossier des thèmes de l'utilisateur.

`app/src-tauri/src/api.rs` ligne 139

```rust
#[tauri::command]
pub fn themes_dir(app: tauri::AppHandle) -> Option<String> {
    let dir = app.path().app_config_dir().ok()?.join("themes");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.to_string_lossy().into_owned())
}
```

### [APP-10] Commande get_settings

Renvoie la vue des réglages.

`app/src-tauri/src/api.rs` ligne 147

```rust
#[tauri::command]
pub fn get_settings(state: tauri::State<'_, App>) -> SettingsView {
    state.view(None)
}
```

### [APP-11] Commande save_output

Valide et enregistre une sortie, range son mot de passe.

`app/src-tauri/src/api.rs` ligne 153

```rust
#[tauri::command]
pub fn save_output(
    state: tauri::State<'_, App>,
    mut output: OutputConfig,
    password: Option<String>,
) -> Result<SettingsView, String> {
    output.validate()?;
    if output.id.is_empty() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
…
```

### [APP-12] Commande delete_output

Supprime une sortie et son mot de passe.

`app/src-tauri/src/api.rs` ligne 179

```rust
#[tauri::command]
pub fn delete_output(state: tauri::State<'_, App>, id: String) -> Result<SettingsView, String> {
    state.secrets.delete(&output_key(&id));
    state.commit(|cfg| cfg.outputs.retain(|o| o.id != id))?;
    Ok(state.view(None))
}
```

### [APP-13] Commande test_output

Teste une sortie sans l'enregistrer.

`app/src-tauri/src/api.rs` ligne 187

```rust
#[tauri::command]
pub async fn test_output(
    state: tauri::State<'_, App>,
    output: OutputConfig,
    password: Option<String>,
) -> Result<String, String> {
    output.validate()?;
    let pw = password
        .filter(|p| !p.is_empty())
        .or_else(|| state.secrets.get(&output_key(&output.id)))
        .ok_or("Saisis le mot de passe de la source.")?;
    tauri::async_runtime::spawn_blocking(move || {
…
```

### [APP-14] Commande save_aura

Enregistre les réglages OBS et leur mot de passe.

`app/src-tauri/src/api.rs` ligne 206

```rust
#[tauri::command]
pub fn save_aura(
    state: tauri::State<'_, App>,
    aura: AuraSettings,
    password: Option<String>,
) -> Result<SettingsView, String> {
    if aura.host.trim().is_empty() || aura.port == 0 {
        return Err("Indique l'adresse et le port du serveur WebSocket d'OBS.".into());
    }
    let warning = state.store_password(AURA_KEY, password);
    state.commit(move |cfg| cfg.aura = aura)?;
    Ok(state.view(warning))
…
```

### [APP-15] Commande test_aura

Teste la connexion à OBS.

`app/src-tauri/src/api.rs` ligne 240

```rust
#[tauri::command]
pub async fn test_aura(
    state: tauri::State<'_, App>,
    aura: AuraSettings,
    password: Option<String>,
) -> Result<String, String> {
    let pw = password
        .filter(|p| !p.is_empty())
        .or_else(|| state.secrets.get(AURA_KEY));
    tauri::async_runtime::spawn_blocking(move || {
        blast_aura::test_connection(&aura, pw.as_deref())
            .map(|i| {
…
```

### [APP-16] Commande save_audio

Enregistre la sortie audio, prise en compte au redémarrage.

`app/src-tauri/src/api.rs` ligne 270

```rust
#[tauri::command]
pub fn save_audio(
    state: tauri::State<'_, App>,
    device: Option<String>,
) -> Result<SettingsView, String> {
    let device = device.filter(|d| !d.is_empty());
    state.commit(move |cfg| cfg.audio.device = device)?;
    Ok(state.view(Some(
        "La nouvelle sortie audio sera utilisée au prochain démarrage de BLAST.".into(),
    )))
}
```

### [APP-17] Commande save_voice

Enregistre les réglages de Voice FX en conservant le choix du micro.

`app/src-tauri/src/api.rs` ligne 283

```rust
#[tauri::command]
pub fn save_voice(state: tauri::State<'_, App>, voice: VoiceSettings) -> Result<(), String> {
    state.commit(move |cfg| {
        let VoiceSettings {
            enabled,
            input_device,
            ..
        } = cfg.voice.clone();
        cfg.voice = VoiceSettings {
            enabled,
            input_device,
            ..voice
…
```

### [APP-18] Commande save_input

Enregistre le micro et son activation, pris en compte au redémarrage.

`app/src-tauri/src/api.rs` ligne 300

```rust
#[tauri::command]
pub fn save_input(
    state: tauri::State<'_, App>,
    enabled: bool,
    device: Option<String>,
) -> Result<SettingsView, String> {
    let device = device.filter(|d| !d.is_empty());
    state.commit(move |cfg| {
        cfg.voice.enabled = enabled;
        cfg.voice.input_device = device;
    })?;
    Ok(state.view(Some(
…
```

### [APP-19] Commande save_pad

Affecte un son à un pad.

`app/src-tauri/src/api.rs` ligne 317

```rust
#[tauri::command]
pub fn save_pad(state: tauri::State<'_, App>, pad: PadConfig) -> Result<(), String> {
    if pad.index >= PAD_COUNT {
        return Err("Pad inexistant.".into());
    }
    if !pad.path.is_file() {
        return Err(format!("Fichier introuvable : {}", pad.path.display()));
    }
    let mut pad = pad;
    if pad.name.trim().is_empty() {
        pad.name = pad
            .path
…
```

### [APP-20] Commande clear_pad

Vide un pad.

`app/src-tauri/src/api.rs` ligne 341

```rust
#[tauri::command]
pub fn clear_pad(state: tauri::State<'_, App>, index: u16) -> Result<(), String> {
    state.commit(move |cfg| cfg.soundboard.pads.retain(|p| p.index != index))
}
```

### [APP-21] Commande save_sfx_volume

Enregistre le volume de la Sound FX.

`app/src-tauri/src/api.rs` ligne 347

```rust
#[tauri::command]
pub fn save_sfx_volume(state: tauri::State<'_, App>, volume_db: f32) -> Result<(), String> {
    state.commit(move |cfg| cfg.soundboard.volume_db = volume_db.clamp(-60.0, 6.0))
}
```

### [APP-22] Commande save_master_preset

Choisit le préréglage du traitement antenne.

`app/src-tauri/src/api.rs` ligne 353

```rust
#[tauri::command]
pub fn save_master_preset(state: tauri::State<'_, App>, preset: String) -> Result<(), String> {
    if !blast_core::MASTER_PRESETS
        .iter()
        .any(|(id, _)| *id == preset)
    {
        return Err(format!("Préréglage inconnu : {preset}"));
    }
    state.commit(move |cfg| cfg.audio.master_preset = preset)
}
```

### [APP-23] Commande save_automation

Enregistre les règles de séparation et la durée de conservation.

`app/src-tauri/src/api.rs` ligne 365

```rust
#[tauri::command]
pub fn save_automation(
    state: tauri::State<'_, App>,
    automation: AutomationSettings,
) -> Result<SettingsView, String> {
    if automation.memory_size == 0 || automation.memory_size > 5000 {
        return Err(
            "La mémoire de l'historique doit être comprise entre 1 et 5000 passages.".into(),
        );
    }
    state.commit(move |cfg| cfg.automation = automation)?;
    Ok(state.view(None))
…
```

### [APP-24] Commande export_history

Exporte en CSV l'historique complet de la base, filtré par catégorie et par période.

`app/src-tauri/src/api.rs` ligne 422

```rust
#[tauri::command]
pub fn export_history(
    state: tauri::State<'_, App>,
    path: String,
    kind: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
) -> Result<usize, String> {
    let kind = match kind.as_deref() {
        None | Some("") | Some("all") => None,
        Some(k) => Some(Kind::parse(k).ok_or_else(|| format!("Catégorie inconnue : {k}"))?),
    };
…
```

### [APP-25] Informations de l'application

Version, dépôt, journal, dossier de réglages et préférences de mise à jour.

`app/src-tauri/src/api.rs` ligne 456

```rust
#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    repository: String,
    releases: String,
    log_path: Option<String>,
    config_dir: Option<String>,
    updates: config::UpdateSettings,
    platform: &'static str,
}
```

### [APP-26] Commande app_info

Renvoie les informations de l'onglet À propos.

`app/src-tauri/src/api.rs` ligne 468

```rust
#[tauri::command]
pub fn app_info(state: tauri::State<'_, App>) -> AppInfo {
    let updates = state
        .config
        .lock()
        .map(|c| c.updates.clone())
        .unwrap_or_default();
    AppInfo {
        version: update::CURRENT,
        repository: format!("https://github.com/{}", update::REPO),
        releases: update::releases_page(),
        log_path: journal::path().map(|p| p.display().to_string()),
…
```

### [APP-27] Commande check_update

Interroge GitHub ; au démarrage, ne signale que les versions plus récentes et non ignorées.

`app/src-tauri/src/api.rs` ligne 487

```rust
#[tauri::command]
pub async fn check_update(
    state: tauri::State<'_, App>,
    automatic: bool,
) -> Result<Option<update::UpdateInfo>, String> {
    let skipped = state
        .config
        .lock()
        .ok()
        .and_then(|c| c.updates.skipped.clone());
    let info = tauri::async_runtime::spawn_blocking(update::check)
        .await
…
```

### [APP-28] Commande save_updates

Enregistre la vérification au démarrage et la version ignorée.

`app/src-tauri/src/api.rs` ligne 522

```rust
#[tauri::command]
pub fn save_updates(
    state: tauri::State<'_, App>,
    check_on_start: Option<bool>,
    skip: Option<String>,
) -> Result<(), String> {
    state.commit(move |cfg| {
        if let Some(v) = check_on_start {
            cfg.updates.check_on_start = v;
        }
        if skip.is_some() {
            cfg.updates.skipped = skip;
…
```

### [APP-29] Commande open_link

Ouvre dans le navigateur un lien du dépôt GitHub, et seulement celui-ci.

`app/src-tauri/src/api.rs` ligne 539

```rust
#[tauri::command]
pub fn open_link(url: String) -> Result<(), String> {
    let allowed = format!("https://github.com/{}", update::REPO);
    if url != allowed && !url.starts_with(&format!("{allowed}/")) {
        return Err("Lien non autorisé.".into());
    }
    open::that_detached(&url).map_err(|e| e.to_string())
}
```

### [APP-30] Commande open_folder

Ouvre le dossier du journal ou des réglages.

`app/src-tauri/src/api.rs` ligne 549

```rust
#[tauri::command]
pub fn open_folder(state: tauri::State<'_, App>, which: String) -> Result<(), String> {
    let dir = match which.as_str() {
        "config" => state.config_path.parent().map(PathBuf::from),
        "log" => journal::path().and_then(|p| p.parent().map(PathBuf::from)),
        _ => None,
    }
    .ok_or("Dossier inconnu.")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open::that_detached(&dir).map_err(|e| e.to_string())
}
```

### [APP-31] Commande save_categories

Enregistre les mots-clés de dossier des jingles et des pubs ; la playlist est reclassée aussitôt.

`app/src-tauri/src/api.rs` ligne 380

```rust
#[tauri::command]
pub fn save_categories(
    state: tauri::State<'_, App>,
    categories: CategorySettings,
) -> Result<SettingsView, String> {
    let clean = |list: Vec<String>| -> Vec<String> {
        list.into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect()
    };
    let categories = CategorySettings {
…
```

### [APP-32] Commande save_jingles

Enregistre le dossier des jingles (vérifié) et leur volume.

`app/src-tauri/src/api.rs` ligne 400

```rust
#[tauri::command]
pub fn save_jingles(
    state: tauri::State<'_, App>,
    dirs: Option<Vec<PathBuf>>,
    volume_db: Option<f32>,
) -> Result<(), String> {
    if let Some(d) = &dirs {
        if let Some(missing) = d.iter().find(|p| !p.is_dir()) {
            return Err(format!("Dossier introuvable : {}", missing.display()));
        }
    }
    state.commit(move |cfg| {
…
```

### [APP-33] Formats d'image de fond

JPEG, PNG, WebP et AVIF.

`app/src-tauri/src/api.rs` ligne 562

```rust
const IMAGE_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "avif"];
```

### [APP-34] Poids maximal de l'image de fond

40 Mo.

`app/src-tauri/src/api.rs` ligne 564

```rust
const MAX_IMAGE_BYTES: u64 = 40 * 1024 * 1024;
```

### [APP-35] Vue de l'apparence

Réglages du verre dépoli et chemin de l'image de fond copiée.

`app/src-tauri/src/api.rs` ligne 567

```rust
#[derive(Serialize)]
pub struct AppearanceView {
    settings: AppearanceSettings,
    image: Option<String>,
}
```

### [APP-36] Calcul de la vue d'apparence

Lit les réglages et vérifie que l'image de fond existe encore ; gère le dossier de BLAST où l'image est copiée et le ménage des anciennes images.

`app/src-tauri/src/api.rs` ligne 582

```rust
fn appearance_view(&self) -> AppearanceView {
    let settings = self
        .config
        .lock()
        .map(|c| c.appearance.clone())
        .unwrap_or_default();
    let image = settings
        .background
        .as_ref()
        .map(|name| self.config_dir().join(name))
        .filter(|p| p.is_file())
        .map(|p| p.display().to_string());
…
```

### [APP-37] Commande get_appearance

Renvoie l'apparence au démarrage de l'interface.

`app/src-tauri/src/api.rs` ligne 611

```rust
#[tauri::command]
pub fn get_appearance(state: tauri::State<'_, App>) -> AppearanceView {
    state.appearance_view()
}
```

### [APP-38] Commande save_appearance

Enregistre activation, flou, opacité et assombrissement, bornés à des valeurs sûres.

`app/src-tauri/src/api.rs` ligne 617

```rust
#[tauri::command]
pub fn save_appearance(
    state: tauri::State<'_, App>,
    appearance: AppearanceSettings,
) -> Result<AppearanceView, String> {
    let appearance = appearance.clamped();
    state.commit(move |cfg| {
        cfg.appearance = AppearanceSettings {
            background: cfg.appearance.background.clone(),
            ..appearance
        }
    })?;
…
```

### [APP-39] Commande set_background

Copie l'image choisie dans le dossier de BLAST sous un nom unique, active le verre et supprime l'image précédente.

`app/src-tauri/src/api.rs` ligne 633

```rust
#[tauri::command]
pub fn set_background(
    state: tauri::State<'_, App>,
    path: PathBuf,
) -> Result<AppearanceView, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| IMAGE_EXTENSIONS.contains(&e.as_str()))
        .ok_or("Format d'image non pris en charge (JPEG, PNG, WebP ou AVIF).")?;
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size > MAX_IMAGE_BYTES {
…
```

### [APP-40] Commande clear_background

Retire l'image de fond ; le verre s'affiche alors sur un dégradé du thème.

`app/src-tauri/src/api.rs` ligne 661

```rust
#[tauri::command]
pub fn clear_background(state: tauri::State<'_, App>) -> Result<AppearanceView, String> {
    state.commit(|cfg| cfg.appearance.background = None)?;
    state.remove_backgrounds(None);
    Ok(state.appearance_view())
}
```

### [APP-41] Commande save_library_dirs

Enregistre les dossiers de la bibliothèque (vérifiés) ; le contrôleur les relit aussitôt.

`app/src-tauri/src/api.rs` ligne 669

```rust
#[tauri::command]
pub fn save_library_dirs(
    state: tauri::State<'_, App>,
    dirs: Vec<PathBuf>,
) -> Result<SettingsView, String> {
    if let Some(missing) = dirs.iter().find(|p| !p.is_dir()) {
        return Err(format!("Dossier introuvable : {}", missing.display()));
    }
    state.commit(move |cfg| cfg.library.dirs = dirs)?;
    Ok(state.view(None))
}
```

### [APP-42] Commande save_rotation

Valide puis enregistre catégories, grilles, planning et grille hors planning.

`app/src-tauri/src/api.rs` ligne 682

```rust
#[tauri::command]
pub fn save_rotation(
    state: tauri::State<'_, App>,
    rotation: blast_library::Rotation,
) -> Result<SettingsView, String> {
    rotation.validate()?;
    let mut rotation = rotation;
    rotation
        .schedule
        .resize(blast_library::SCHEDULE_SLOTS, String::new());
    state.commit(move |cfg| cfg.rotation = rotation)?;
    Ok(state.view(None))
…
```

### [APP-43] Commande rotation_counts

Compte les titres de la bibliothèque qui correspondent à des règles de catégorie, pour l'aperçu en direct.

`app/src-tauri/src/api.rs` ligne 697

```rust
#[tauri::command]
pub async fn rotation_counts(
    state: tauri::State<'_, App>,
    rules: Vec<blast_library::CategoryRule>,
) -> Result<Vec<usize>, String> {
    let db = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let tracks = blast_library::SqliteStore::open(&db)
            .and_then(|l| l.tracks())
            .map_err(|e| e.to_string())?;
        let lists: blast_library::PlaylistSets = rules
            .iter()
…
```

### [APP-44] Fermeture

À la sortie, demande au contrôleur d'écrire la playlist et attend jusqu'à 5 s.

`app/src-tauri/src/main.rs` ligne 145

```rust
fn on_event(handle: &tauri::AppHandle, event: tauri::RunEvent) {
    if !matches!(event, tauri::RunEvent::Exit) {
        return;
    }
    let Some(state) = handle.try_state::<api::App>() else {
        return;
    };
    let servers = state
        .config
        .lock()
        .map(|c| c.icecast.servers.clone())
        .unwrap_or_default();
…
```

### [APP-45] Enregistrement du chat

Enregistre les réglages du chat ; le jeton va au trousseau, sans le préfixe oauth:, ou en est retiré.

`app/src-tauri/src/api.rs` ligne 221

```rust
#[tauri::command]
pub fn save_chat(
    state: tauri::State<'_, App>,
    chat: crate::config::ChatSettings,
    token: Option<String>,
    forget_token: Option<bool>,
) -> Result<SettingsView, String> {
    if forget_token.unwrap_or(false) {
        state.secrets.delete(crate::secrets::CHAT_KEY);
    }
    let warning = state.store_password(
        crate::secrets::CHAT_KEY,
…
```


## CFG

Réglages enregistrés (config.toml)

### [CFG-01] Réglages audio

Sortie audio choisie et préréglage du traitement antenne.

`app/src-tauri/src/config.rs` ligne 7

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub device: Option<String>,
    pub master_preset: String,
}
```

### [CFG-02] Réglages Voice FX

Micro, gains, ducking, égaliseur, passe-haut, retour et préréglage de voix.

`app/src-tauri/src/config.rs` ligne 24

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub enabled: bool,
    pub input_device: Option<String>,
    pub mic_gain_db: f32,
    pub music_gain_db: f32,
    pub duck: bool,
    pub duck_db: f32,
    pub duck_attack_ms: u32,
    pub duck_release_ms: u32,
    pub low_db: f32,
…
```

### [CFG-03] Pad

Numéro, nom, fichier et couleur d'un pad.

`app/src-tauri/src/config.rs` ligne 65

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PadConfig {
    pub index: u16,
    pub name: String,
    pub path: PathBuf,
    pub color: String,
}
```

### [CFG-04] Réglages Sound FX

Volume général et liste des pads.

`app/src-tauri/src/config.rs` ligne 75

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundboardSettings {
    pub volume_db: f32,
    pub pads: Vec<PadConfig>,
}
```

### [CFG-05] Nombre de pads

36 pads, en trois pages de 12.

`app/src-tauri/src/config.rs` ligne 83

```rust
pub const PAD_COUNT: u16 = 36;
```

### [CFG-06] Réglages d'automation

Règles de séparation, durée de conservation de l'historique et nombre de passages gardés en mémoire par catégorie (200 par défaut).

`app/src-tauri/src/config.rs` ligne 86

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutomationSettings {
    pub rules: blast_library::SeparationRules,
    pub history_days: u32,
    pub memory_size: u32,
}
```

### [CFG-07] Réglages de mise à jour

Vérification au démarrage et version ignorée.

`app/src-tauri/src/config.rs` ligne 139

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    pub check_on_start: bool,
    pub skipped: Option<String>,
}
```

### [CFG-08] Réglages complets

Contenu de config.toml ; aucun mot de passe n'y figure.

`app/src-tauri/src/config.rs` ligne 185

```rust

```

### [CFG-09] Chargement

Lit config.toml ; un fichier illisible est mis de côté et remplacé par les valeurs par défaut.

`app/src-tauri/src/config.rs` ligne 288

```rust
pub fn load(path: &Path) -> (AppConfig, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (AppConfig::default(), None);
    };
    match toml::from_str(&text) {
        Ok(cfg) => (cfg, None),
        Err(e) => {
            let backup: PathBuf = path.with_extension("toml.bad");
            let _ = std::fs::rename(path, &backup);
            (
                AppConfig::default(),
                Some(format!(
…
```

### [CFG-10] Enregistrement

Écrit config.toml via un fichier temporaire renommé, pour ne jamais laisser de fichier à moitié écrit.

`app/src-tauri/src/config.rs` ligne 309

```rust
pub fn save(path: &Path, cfg: &AppConfig) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
```

### [CFG-11] Réglages des catégories

Mots-clés de dossier qui classent un fichier en jingle ou en pub.

`app/src-tauri/src/config.rs` ligne 105

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategorySettings {
    pub jingle_keywords: Vec<String>,
    pub pub_keywords: Vec<String>,
}
```

### [CFG-12] Réglages des jingles

Dossier de la box à jingles et volume des jingles.

`app/src-tauri/src/config.rs` ligne 122

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JingleSettings {
    pub dirs: Vec<PathBuf>,
    pub volume_db: f32,
}
```

### [CFG-13] Réglages d'apparence

Verre dépoli activé ou non, flou, opacité des panneaux, assombrissement du fond et image de fond.

`app/src-tauri/src/config.rs` ligne 193

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub glass: bool,
    pub blur: f64,
    pub opacity: f64,
    pub dim: f64,
    pub background: Option<String>,
}
```

### [CFG-14] Réglages de la bibliothèque

Dossiers de musique lus pour alimenter les grilles.

`app/src-tauri/src/config.rs` ligne 186

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    pub dirs: Vec<PathBuf>,
}
```

### [CFG-15] Profil d'un serveur Icecast

Nom, local ou distant, dossier d'installation, exécutable, fichier de configuration, adresse, port, HTTPS, identifiant admin, lancement au démarrage et arrêt à la fermeture. Le mot de passe admin est dans le trousseau.

`app/src-tauri/src/config.rs` ligne 225

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IceServerProfile {
    pub id: String,
    pub name: String,
    pub remote: bool,
    pub install_dir: Option<PathBuf>,
    pub exe: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub host: String,
    pub port: u16,
    pub https: bool,
…
```

### [CFG-16] Serveurs Icecast

Liste des profils de serveurs Icecast.

`app/src-tauri/src/config.rs` ligne 262

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IcecastSettings {
    pub servers: Vec<IceServerProfile>,
}
```

### [CFG-17] Réglages du chat

Chaîne, compte qui écrit, connexion au démarrage, annonce des titres, commandes actives et limite de demandes par viewer.

`app/src-tauri/src/config.rs` ligne 156

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatSettings {
    pub channel: String,
    pub bot_login: String,
    pub autoconnect: bool,
    pub announce: bool,
    pub cmd_np: bool,
    pub cmd_request: bool,
    pub cmd_sfx: bool,
    pub request_limit: u32,
}
```


## SEC

Mots de passe dans le trousseau du système

### [SEC-01] Nom du service

Identifiant sous lequel les mots de passe sont rangés dans le trousseau.

`app/src-tauri/src/secrets.rs` ligne 4

```rust
const SERVICE: &str = "fr.black-lab.blast";
```

### [SEC-02] Lieu de rangement

Trousseau du système ou mémoire seule (avec la raison).

`app/src-tauri/src/secrets.rs` ligne 7

```rust
pub enum Stored {
    Keyring,
    Memory(String),
}
```

### [SEC-03] Trousseau

Lecture et écriture des mots de passe dans le trousseau, avec repli en mémoire.

`app/src-tauri/src/secrets.rs` ligne 13

```rust
#[derive(Default)]
pub struct Secrets {
    memory: Mutex<HashMap<String, String>>,
    last_error: Mutex<Option<String>>,
}
```

### [SEC-04] Clé d'une sortie

Nom de la clé du mot de passe d'une sortie de diffusion.

`app/src-tauri/src/secrets.rs` ligne 20

```rust
pub fn output_key(id: &str) -> String {
    format!("output:{id}")
}
```

### [SEC-05] Clé OBS

Nom de la clé du mot de passe OBS.

`app/src-tauri/src/secrets.rs` ligne 25

```rust
pub const AURA_KEY: &str = "aura";
```

### [SEC-06] Opérations du trousseau

Lire, tester, écrire, supprimer et décrire le lieu de rangement.

`app/src-tauri/src/secrets.rs` ligne 36

```rust
impl Secrets {
    pub fn get(&self, key: &str) -> Option<String> {
        if let Some(v) = self.memory.lock().ok()?.get(key) {
            return Some(v.clone());
        }
        keyring::Entry::new(SERVICE, key)
            .and_then(|e| e.get_password())
            .ok()
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some_and(|p| !p.is_empty())
…
```

### [SEC-07] Clé d'un serveur Icecast

Nom du mot de passe admin d'un serveur Icecast dans le trousseau.

`app/src-tauri/src/secrets.rs` ligne 31

```rust
pub fn ice_key(id: &str) -> String {
    format!("icecast:{id}")
}
```

### [SEC-08] Clé du jeton Twitch

Nom de l'entrée du trousseau qui garde le jeton du chat.

`app/src-tauri/src/secrets.rs` ligne 28

```rust
pub const CHAT_KEY: &str = "twitch";
```


## ICM

Gestion des serveurs Icecast côté application

### [ICM-01] Sauvegardes gardées

Nombre de copies .bak gardées pour chaque icecast.xml modifié par BLAST.

`app/src-tauri/src/icecast.rs` ligne 18

```rust
const BACKUPS_KEPT: usize = 10;
```

### [ICM-02] Gestionnaire des serveurs

Processus Icecast suivis par BLAST, dossier de données (PID, console, journaux des copies) et dossier de réglages (copies de icecast.xml).

`app/src-tauri/src/icecast.rs` ligne 21

```rust
pub struct IceManager {
    running: Mutex<HashMap<String, Process>>,
    data_dir: PathBuf,
    config_dir: PathBuf,
}
```

### [ICM-03] Identifiant sûr

Lettres, chiffres, tirets et soulignés seulement : l'identifiant sert de nom de dossier.

`app/src-tauri/src/icecast.rs` ligne 28

```rust
fn safe_id(id: &str) -> Result<&str, String> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Identifiant de serveur invalide.".into());
    }
    Ok(id)
}
```

### [ICM-04] Dossier de travail

Dossier d'installation s'il existe, sinon le dossier de l'exécutable (son parent s'il s'appelle bin). Les chemins relatifs de icecast.xml en partent.

`app/src-tauri/src/icecast.rs` ligne 41

```rust
fn workdir(p: &IceServerProfile) -> Option<PathBuf> {
    if let Some(d) = p.install_dir.as_ref().filter(|d| d.is_dir()) {
        return Some(d.clone());
    }
    let parent = p.exe.as_ref()?.parent()?;
    let is_bin = parent
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("bin"));
    Some(if is_bin {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
…
```

### [ICM-05] État d'un serveur local

En marche et lancé par BLAST ou non ; reprend le processus noté dans icecast.pid après un redémarrage de BLAST.

`app/src-tauri/src/icecast.rs` ligne 86

```rust
pub fn state(&self, id: &str) -> (bool, bool) {
    let Ok(mut map) = self.running.lock() else {
        return (false, false);
    };
    if let Some(p) = map.get_mut(id) {
        if p.alive() {
            return (true, p.owned());
        }
        map.remove(id);
        let _ = std::fs::remove_file(self.pid_file(id));
    }
    let pid = std::fs::read_to_string(self.pid_file(id))
…
```

### [ICM-06] Lancement

Vérifie l'exécutable et la configuration, la valide, crée le dossier des journaux, lance Icecast et note son PID. Refusé pour un serveur distant.

`app/src-tauri/src/icecast.rs` ligne 108

```rust
pub fn start(&self, p: &IceServerProfile) -> Result<String, String> {
    if p.remote {
        return Err(
            "Le lancement d'un serveur distant demandera une connexion SSH (prochaine étape)."
                .into(),
        );
    }
    if self.state(&p.id).0 {
        return Ok("Icecast tourne déjà.".into());
    }
    let exe = p
        .exe
…
```

### [ICM-07] Arrêt

Arrête le processus lancé ou repris par BLAST ; un Icecast lancé comme service Windows n'est pas touché.

`app/src-tauri/src/icecast.rs` ligne 153

```rust
pub fn stop(&self, id: &str) -> Result<String, String> {
    let _ = self.state(id);
    let proc = self.running.lock().ok().and_then(|mut m| m.remove(id));
    let _ = std::fs::remove_file(self.pid_file(id));
    match proc {
        Some(mut p) => {
            p.stop()?;
            journal::info(format!("icecast {id} : arrêté"));
            Ok("Icecast arrêté.".into())
        }
        None => Err("BLAST ne trouve pas de processus Icecast lancé par lui. S'il tourne comme service Windows, arrête-le depuis les Services.".into()),
    }
…
```

### [ICM-08] Arrêt à la fermeture

À la fermeture de BLAST, arrête les serveurs locaux réglés ainsi et lancés par BLAST.

`app/src-tauri/src/icecast.rs` ligne 168

```rust
pub fn stop_on_exit(&self, profiles: &[IceServerProfile]) {
    for p in profiles.iter().filter(|p| p.stop_on_exit && !p.remote) {
        if self.state(&p.id) == (true, true) {
            let _ = self.stop(&p.id);
        }
    }
}
```

### [ICM-09] Vue d'un serveur

Profil, présence d'un mot de passe admin et indication d'une configuration copiée dans BLAST.

`app/src-tauri/src/icecast.rs` ligne 178

```rust
#[derive(Serialize)]
pub struct ServerView {
    #[serde(flatten)]
    profile: IceServerProfile,
    has_password: bool,
    managed: bool,
}
```

### [ICM-10] Vue de la fenêtre Icecast

Serveurs, installations détectées, emplacement des mots de passe et avertissement éventuel.

`app/src-tauri/src/icecast.rs` ligne 187

```rust
#[derive(Serialize)]
pub struct IceView {
    servers: Vec<ServerView>,
    detected: Vec<Install>,
    secret_store: String,
    warning: Option<String>,
}
```

### [ICM-11] État renvoyé à l'interface

Processus en marche, lancé par BLAST, serveur joignable, version, statistiques et message d'erreur.

`app/src-tauri/src/icecast.rs` ligne 196

```rust
#[derive(Serialize, Default)]
pub struct StatusView {
    running: bool,
    owned: bool,
    reachable: bool,
    server_id: Option<String>,
    stats: Option<ServerStats>,
    error: Option<String>,
    checked_at: u64,
}
```

### [ICM-12] Vue de la configuration

Configuration, chemin, droit d'écriture, copie gérée par BLAST, nombre de sauvegardes, avertissements et message.

`app/src-tauri/src/icecast.rs` ligne 208

```rust
#[derive(Serialize)]
pub struct ConfigView {
    config: IceConfig,
    path: String,
    writable: bool,
    managed: bool,
    backups: usize,
    warnings: Vec<String>,
    message: Option<String>,
}
```

### [ICM-13] Construction de la vue

Assemble la vue de la fenêtre Icecast à partir des réglages et du trousseau.

`app/src-tauri/src/icecast.rs` ligne 267

```rust
fn ice_view(&self, warning: Option<String>) -> IceView {
    let servers = self
        .config
        .lock()
        .map(|c| c.icecast.servers.clone())
        .unwrap_or_default();
    IceView {
        servers: servers
            .into_iter()
            .map(|p| ServerView {
                has_password: self.secrets.has(&ice_key(&p.id)),
                managed: p
…
```

### [ICM-14] Construction de la vue de configuration

Charge icecast.xml d'un serveur local ; un serveur distant demandera SSH.

`app/src-tauri/src/icecast.rs` ligne 292

```rust
fn ice_config_view(
    &self,
    p: &IceServerProfile,
    warnings: Vec<String>,
    message: Option<String>,
) -> Result<ConfigView, String> {
    if p.remote {
        return Err("La configuration d'un serveur distant se modifiera par SSH (prochaine étape). Le tableau de bord, les auditeurs et les actions admin fonctionnent déjà.".into());
    }
    let path = p
        .config_path
        .clone()
…
```

### [ICM-15] Commande ice_list

Renvoie la vue de la fenêtre Icecast.

`app/src-tauri/src/icecast.rs` ligne 345

```rust
#[tauri::command]
pub fn ice_list(state: tauri::State<'_, App>) -> IceView {
    state.ice_view(None)
}
```

### [ICM-16] Commande ice_detect

Cherche une installation dans un dossier donné, ou aux emplacements habituels.

`app/src-tauri/src/icecast.rs` ligne 351

```rust
#[tauri::command]
pub fn ice_detect(dir: Option<PathBuf>) -> Vec<Install> {
    match dir {
        Some(d) => blast_ice::find_in(&d).into_iter().collect(),
        None => blast_ice::detect(),
    }
}
```

### [ICM-17] Commande ice_save_server

Valide le profil, crée son identifiant, cherche l'exécutable, reprend port, identifiant et mot de passe admin dans icecast.xml, range le mot de passe dans le trousseau, prévient si l'admin d'un hôte distant passe en HTTP.

`app/src-tauri/src/icecast.rs` ligne 360

```rust
#[tauri::command]
pub fn ice_save_server(
    state: tauri::State<'_, App>,
    server: IceServerProfile,
    password: Option<String>,
) -> Result<IceView, String> {
    let mut server = server;
    server.name = server.name.trim().to_string();
    server.host = server.host.trim().to_string();
    if server.name.is_empty() {
        return Err("Donne un nom à ce serveur.".into());
    }
…
```

### [ICM-18] Commande ice_delete_server

Arrête le serveur s'il a été lancé par BLAST, efface son mot de passe et son profil ; icecast.xml n'est pas supprimé.

`app/src-tauri/src/icecast.rs` ligne 433

```rust
#[tauri::command]
pub fn ice_delete_server(state: tauri::State<'_, App>, id: String) -> Result<IceView, String> {
    safe_id(&id)?;
    if state.ice.state(&id) == (true, true) {
        let _ = state.ice.stop(&id);
    }
    state.secrets.delete(&ice_key(&id));
    state.commit(|cfg| cfg.icecast.servers.retain(|s| s.id != id))?;
    Ok(state.ice_view(None))
}
```

### [ICM-19] Commande ice_status

Statistiques admin, sinon sonde publique, avec un message adapté (arrêté, en démarrage, mot de passe manquant ou refusé).

`app/src-tauri/src/icecast.rs` ligne 445

```rust
#[tauri::command]
pub async fn ice_status(app: tauri::AppHandle, id: String) -> Result<StatusView, String> {
    let state = app.state::<App>();
    let p = state.ice_profile(&id)?;
    let ep = state.ice_endpoint(&p);
    let has_password = !ep.password.is_empty();
    let app2 = app.clone();
    blocking(move || {
        let state = app2.state::<App>();
        let (running, owned) = if p.remote {
            (false, false)
        } else {
…
```

### [ICM-20] Commande ice_power

Démarre, arrête ou redémarre un serveur local.

`app/src-tauri/src/icecast.rs` ligne 509

```rust
#[tauri::command]
pub async fn ice_power(
    app: tauri::AppHandle,
    id: String,
    action: String,
) -> Result<String, String> {
    let p = app.state::<App>().ice_profile(&id)?;
    blocking(move || {
        let state = app.state::<App>();
        match action.as_str() {
            "start" => state.ice.start(&p),
            "stop" => state.ice.stop(&p.id),
…
```

### [ICM-21] Commande ice_get_config

Renvoie la vue de la configuration d'un serveur.

`app/src-tauri/src/icecast.rs` ligne 535

```rust
#[tauri::command]
pub fn ice_get_config(state: tauri::State<'_, App>, id: String) -> Result<ConfigView, String> {
    let p = state.ice_profile(&id)?;
    state.ice_config_view(&p, Vec::new(), None)
}
```

### [ICM-22] Commande ice_save_config

Valide, vérifie le droit d'écriture, applique au document, relit le résultat, écrit avec sauvegarde, recopie identifiant, mot de passe admin et port dans le profil, puis redémarre si demandé.

`app/src-tauri/src/icecast.rs` ligne 542

```rust
#[tauri::command]
pub async fn ice_save_config(
    app: tauri::AppHandle,
    id: String,
    config: IceConfig,
    restart: bool,
) -> Result<ConfigView, String> {
    blocking(move || {
        let state = app.state::<App>();
        let p = state.ice_profile(&id)?;
        let path = p
            .config_path
…
```

### [ICM-23] Commande ice_adopt_config

Copie icecast.xml dans le dossier de réglages de BLAST avec des chemins absolus, pour le modifier sans droits administrateur ; l'original n'est pas touché.

`app/src-tauri/src/icecast.rs` ligne 593

```rust
#[tauri::command]
pub fn ice_adopt_config(state: tauri::State<'_, App>, id: String) -> Result<ConfigView, String> {
    let p = state.ice_profile(&id)?;
    let src = p
        .config_path
        .clone()
        .ok_or("Aucun fichier de configuration à copier.")?;
    let target = state.ice.managed_config(&p.id);
    if src == target {
        return state.ice_config_view(&p, Vec::new(), None);
    }
    let wd = workdir(&p).ok_or("Dossier d'installation introuvable.")?;
…
```

### [ICM-24] Commande ice_new_config

Crée une configuration neuve à mots de passe aléatoires dans le dossier de réglages de BLAST, avec les pages web et admin de l'installation.

`app/src-tauri/src/icecast.rs` ligne 626

```rust
#[tauri::command]
pub fn ice_new_config(state: tauri::State<'_, App>, id: String) -> Result<ConfigView, String> {
    let p = state.ice_profile(&id)?;
    if p.remote {
        return Err("Disponible pour un serveur local seulement.".into());
    }
    let target = state.ice.managed_config(&p.id);
    if target.exists() {
        return Err(format!(
            "Une configuration créée par BLAST existe déjà : {}",
            target.display()
        ));
…
```

### [ICM-25] Commande ice_listeners

Liste les auditeurs d'un mountpoint.

`app/src-tauri/src/icecast.rs` ligne 681

```rust
#[tauri::command]
pub async fn ice_listeners(
    app: tauri::AppHandle,
    id: String,
    mount: String,
) -> Result<Vec<Listener>, String> {
    let state = app.state::<App>();
    let ep = state.ice_endpoint(&state.ice_profile(&id)?);
    blocking(move || ep.listeners(&mount).map_err(|e| e.to_string())).await
}
```

### [ICM-26] Commande ice_admin

Exécute une action admin et la note dans le journal de BLAST.

`app/src-tauri/src/icecast.rs` ligne 693

```rust
#[tauri::command]
pub async fn ice_admin(
    app: tauri::AppHandle,
    id: String,
    action: AdminAction,
) -> Result<String, String> {
    let state = app.state::<App>();
    let p = state.ice_profile(&id)?;
    let ep = state.ice_endpoint(&p);
    let name = p.name.clone();
    blocking(move || {
        let r = ep.run(&action).map_err(|e| e.to_string());
…
```

### [ICM-27] Commande ice_log

Fin du journal d'erreurs, d'accès ou de la console d'Icecast.

`app/src-tauri/src/icecast.rs` ligne 714

```rust
#[tauri::command]
pub fn ice_log(
    state: tauri::State<'_, App>,
    id: String,
    which: String,
    lines: Option<usize>,
) -> Result<String, String> {
    let p = state.ice_profile(&id)?;
    let lines = lines.unwrap_or(300).clamp(10, 5000);
    let path = if which == "console" {
        state.ice.console(&p.id)
    } else {
…
```

### [ICM-28] Commande ice_link_output

Ajoute dans Diffusion une sortie Icecast vers un mountpoint, avec son mot de passe (propre ou celui des sources) et sa présentation.

`app/src-tauri/src/icecast.rs` ligne 745

```rust
#[tauri::command]
pub fn ice_link_output(
    state: tauri::State<'_, App>,
    id: String,
    mount: String,
) -> Result<String, String> {
    let p = state.ice_profile(&id)?;
    let cfg = p
        .config_path
        .as_ref()
        .and_then(|c| IceDocument::load(c).ok())
        .map(|d| d.read());
…
```

### [ICM-29] Lancement automatique

Au démarrage de BLAST, lance dans un fil séparé les serveurs locaux réglés ainsi.

`app/src-tauri/src/icecast.rs` ligne 845

```rust
pub fn autostart(app: &tauri::AppHandle) {
    let state = app.state::<App>();
    let servers: Vec<IceServerProfile> = state
        .config
        .lock()
        .map(|c| c.icecast.servers.clone())
        .unwrap_or_default();
    let todo: Vec<IceServerProfile> = servers
        .into_iter()
        .filter(|s| s.autostart && !s.remote)
        .collect();
    if todo.is_empty() {
…
```

### [ICM-30] Commande ice_open_web

Ouvre la page d'état du serveur dans le navigateur.

`app/src-tauri/src/icecast.rs` ligne 828

```rust
#[tauri::command]
pub fn ice_open_web(state: tauri::State<'_, App>, id: String) -> Result<(), String> {
    let p = state.ice_profile(&id)?;
    let host = if matches!(p.host.as_str(), "0.0.0.0" | "::") {
        "localhost".to_string()
    } else {
        p.host.clone()
    };
    let url = Endpoint {
        host,
        ..state.ice_endpoint(&p)
    }
…
```


## JRN

Journal de diagnostic (blast.log)

### [JRN-01] Taille maximale

Au-delà de 1 Mo, le journal est renommé en blast.log.1 au démarrage.

`app/src-tauri/src/journal.rs` ligne 6

```rust
const MAX_BYTES: u64 = 1024 * 1024;
```

### [JRN-02] Journal

Fichier ouvert en ajout et son chemin.

`app/src-tauri/src/journal.rs` ligne 9

```rust
struct Journal {
    file: Mutex<File>,
    path: PathBuf,
}
```

### [JRN-03] Ouverture du journal

Crée le dossier, fait tourner le fichier trop gros et note le démarrage.

`app/src-tauri/src/journal.rs` ligne 17

```rust
pub fn init(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
…
```

### [JRN-04] Chemin du journal

Chemin du fichier, pour l'onglet À propos.

`app/src-tauri/src/journal.rs` ligne 41

```rust
pub fn path() -> Option<PathBuf> {
    JOURNAL.get().map(|j| j.path.clone())
}
```

### [JRN-05] Écriture

Ajoute une ligne horodatée avec son niveau ; sans journal ouvert, écrit sur la sortie d'erreur.

`app/src-tauri/src/journal.rs` ligne 46

```rust
pub fn log(level: &str, message: &str) {
    let line = format!(
        "{} [{}] {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        level.to_uppercase(),
        message
    );
    match JOURNAL.get().and_then(|j| j.file.lock().ok()) {
        Some(mut f) => {
            let _ = f.write_all(line.as_bytes());
        }
        None => eprint!("{line}"),
…
```

### [JRN-06] Niveau info

Raccourci pour une information.

`app/src-tauri/src/journal.rs` ligne 62

```rust
pub fn info(message: impl AsRef<str>) {
    log("info", message.as_ref());
}
```

### [JRN-07] Niveau avertissement

Raccourci pour un avertissement.

`app/src-tauri/src/journal.rs` ligne 67

```rust
pub fn warn(message: impl AsRef<str>) {
    log("warn", message.as_ref());
}
```

### [JRN-08] Niveau erreur

Raccourci pour une erreur.

`app/src-tauri/src/journal.rs` ligne 72

```rust
pub fn error(message: impl AsRef<str>) {
    log("error", message.as_ref());
}
```


## UPD

Vérification des mises à jour sur GitHub

### [UPD-01] Dépôt

Dépôt GitHub interrogé.

`app/src-tauri/src/update.rs` ligne 5

```rust
pub const REPO: &str = "D-Goth/BLAST-Broadcaster";
```

### [UPD-02] Version installée

Version de l'application, issue du workspace.

`app/src-tauri/src/update.rs` ligne 7

```rust
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
```

### [UPD-03] Version publiée

Champs utiles de la réponse de l'API GitHub Releases.

`app/src-tauri/src/update.rs` ligne 10

```rust
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    name: Option<String>,
    html_url: String,
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}
```

### [UPD-04] Résultat de vérification

Version installée, dernière version publiée, nouveauté, lien et notes.

`app/src-tauri/src/update.rs` ligne 24

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub newer: bool,
    pub name: String,
    pub url: String,
    pub notes: String,
    pub published: Option<String>,
}
```

### [UPD-05] Page des versions

Adresse de la page des versions sur GitHub.

`app/src-tauri/src/update.rs` ligne 50

```rust
pub fn releases_page() -> String {
    format!("https://github.com/{REPO}/releases")
}
```

### [UPD-06] Numéro de version

Majeur, mineur, correctif et stabilité, comparables entre eux.

`app/src-tauri/src/update.rs` ligne 55

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    stable: bool,
}
```

### [UPD-07] Lecture d'un numéro

Lit « v1.2.3 », « 1.2 » ou « 1.2.3-rc.1 ».

`app/src-tauri/src/update.rs` ligne 64

```rust
pub fn parse_version(text: &str) -> Option<Version> {
    let text = text.trim().trim_start_matches(['v', 'V']);
    let text = text.split('+').next().unwrap_or_default();
    let (core, pre) = match text.split_once('-') {
        Some((c, _)) => (c, true),
        None => (text, false),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    if parts.next().is_some() {
…
```

### [UPD-08] Comparaison

Indique si une version est plus récente que la version installée.

`app/src-tauri/src/update.rs` ligne 87

```rust
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(c), Some(cur)) => c > cur,
        _ => false,
    }
}
```

### [UPD-09] Évaluation d'une version

Ignore brouillons et préversions, puis compare à la version installée.

`app/src-tauri/src/update.rs` ligne 95

```rust
fn evaluate(release: Release) -> UpdateInfo {
    if release.draft || release.prerelease {
        return UpdateInfo::none();
    }
    let latest = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    UpdateInfo {
        current: CURRENT.into(),
        newer: is_newer(&latest, CURRENT),
        name: release
            .name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| release.tag_name.clone()),
…
```

### [UPD-10] Vérification

Interroge l'API GitHub avec le magasin de certificats du système ; aucune version publiée n'est pas une erreur.

`app/src-tauri/src/update.rs` ligne 115

```rust
pub fn check() -> Result<UpdateInfo, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
…
```


## PL

Playlist côté application

### [PL-01] Entrée de playlist

Fichier, tags, durée, erreur de lecture éventuelle et motif d'attente des règles.

`app/src-tauri/src/playlist.rs` ligne 9

```rust
#[derive(Debug, Clone, Default, Serialize)]
pub struct Entry {
    pub id: u64,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    #[serde(skip_serializing)]
    pub album: String,
    #[serde(skip_serializing)]
    pub genre: String,
    #[serde(skip_serializing)]
    pub year: Option<i32>,
…
```

### [PL-02] Playlist

Liste ordonnée d'entrées avec identifiants uniques.

`app/src-tauri/src/playlist.rs` ligne 57

```rust
#[derive(Debug, Default)]
pub struct Playlist {
    entries: Vec<Entry>,
    next_id: u64,
}
```

### [PL-03] Opérations de playlist

Insérer à une position, retirer, déplacer, vider et totaliser la durée.

`app/src-tauri/src/playlist.rs` ligne 64

```rust
impl Playlist {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn total_secs(&self) -> f64 {
        self.entries.iter().filter_map(|e| e.duration).sum()
    }

    pub fn insert(&mut self, mut items: Vec<Entry>, index: Option<usize>) {
        for e in items.iter_mut() {
            self.next_id += 1;
…
```

### [PL-04] Parcours de dossier

Parcourt un dossier récursivement, dans l'ordre alphabétique.

`app/src-tauri/src/playlist.rs` ligne 183

```rust
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
    items.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    for p in items {
        if p.is_dir() {
            walk(&p, out);
        } else if is_audio_path(&p) {
            out.push(p);
        }
…
```

### [PL-05] Développement des chemins

Remplace les dossiers par les fichiers audio qu'ils contiennent et les playlists M3U par leurs titres existants.

`app/src-tauri/src/playlist.rs` ligne 199

```rust
pub fn expand(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            walk(p, &mut out);
        } else if is_m3u(p) {
            out.extend(read_m3u(p).into_iter().filter(|t| t.is_file()));
        } else if is_audio_path(p) {
            out.push(p.clone());
        }
    }
    out
…
```

### [PL-06] Création d'entrée

Sonde un fichier ; un fichier illisible devient une entrée signalée en erreur.

`app/src-tauri/src/playlist.rs` ligne 352

```rust
pub fn make_entry(path: &Path, cats: &CategorySettings) -> Entry {
    let kind = classify(path, cats);
    match probe_file(path) {
        Ok(info) => Entry {
            id: 0,
            path: path.to_path_buf(),
            title: info.display_title(),
            artist: info.artist.unwrap_or_default(),
            album: info.album.unwrap_or_default(),
            genre: info.genre.unwrap_or_default(),
            year: info.year,
            duration: info.duration_secs,
…
```

### [PL-07] Normalisation d'un mot

Minuscules et accents retirés, pour comparer les noms de dossiers.

`app/src-tauri/src/playlist.rs` ligne 269

```rust
fn fold(word: &str) -> String {
    word.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'À' | 'Â' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'î' | 'ï' | 'Î' | 'Ï' => 'i',
            'ô' | 'ö' | 'Ô' | 'Ö' => 'o',
            'ù' | 'û' | 'ü' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            c => c.to_ascii_lowercase(),
        })
        .collect()
…
```

### [PL-08] Mot-clé dans un dossier

Le nom de dossier contient-il un des mots-clés, au singulier ou au pluriel ?

`app/src-tauri/src/playlist.rs` ligne 284

```rust
fn folder_matches(folder: &str, keywords: &[String]) -> bool {
    let words: Vec<String> = folder
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(fold)
        .collect();
    keywords.iter().map(|k| fold(k.trim())).any(|k| {
        !k.is_empty()
            && words
                .iter()
                .any(|w| *w == k || w.strip_suffix('s') == Some(k.as_str()))
    })
…
```

### [PL-09] Classement d'un fichier

Jingle, pub ou morceau d'après le dossier le plus proche qui porte un mot-clé.

`app/src-tauri/src/playlist.rs` ligne 299

```rust
pub fn classify(path: &Path, cats: &CategorySettings) -> Kind {
    let folders: Vec<String> = path
        .parent()
        .into_iter()
        .flat_map(|p| p.components())
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    for folder in folders.iter().rev() {
        if folder_matches(folder, &cats.jingle_keywords) {
            return Kind::Jingle;
        }
        if folder_matches(folder, &cats.pub_keywords) {
…
```

### [PL-10] Étiquette de grille

Nom et couleur de la catégorie ou de l'émission qui a ajouté un titre ; « fixed » marque un titre d'émission, joué dans l'ordre et exempté des règles de séparation.

`app/src-tauri/src/playlist.rs` ligne 31

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SlotTag {
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub fixed: bool,
}
```

### [PL-11] Date de modification d'un fichier

Date de modification en secondes, pour le scan incrémental.

`app/src-tauri/src/playlist.rs` ligne 318

```rust
pub fn file_mtime(path: &Path) -> Option<i64> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}
```

### [PL-12] Entrée depuis la bibliothèque

Construit une entrée de playlist à partir d'un titre de la bibliothèque, sans relire le fichier.

`app/src-tauri/src/playlist.rs` ligne 327

```rust
pub fn from_library(t: &blast_library::LibraryTrack, slot: SlotTag) -> Entry {
    let path = PathBuf::from(&t.path);
    let title = if t.title.trim().is_empty() {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        t.title.clone()
    };
    Entry {
        id: 0,
        path,
…
```

### [PL-13] Mélange

Mélange de Fisher-Yates de toute la playlist.

`app/src-tauri/src/playlist.rs` ligne 112

```rust
pub fn shuffle(&mut self, roll: &mut impl FnMut() -> f64) {
    let n = self.entries.len();
    for i in (1..n).rev() {
        let j = ((roll() * (i + 1) as f64) as usize).min(i);
        self.entries.swap(i, j);
    }
}
```

### [PL-14] Mélange en espaçant les artistes

Mélange puis regroupe par artiste et pioche à chaque fois dans le plus gros groupe qui n'est pas celui du titre précédent : les titres d'un même artiste sont répartis au plus loin ; les titres sans artiste ne sont pas contraints.

`app/src-tauri/src/playlist.rs` ligne 121

```rust
pub fn shuffle_spread(&mut self, roll: &mut impl FnMut() -> f64) {
    self.shuffle(roll);
    let mut groups: Vec<(String, Vec<Entry>)> = Vec::new();
    for (i, e) in std::mem::take(&mut self.entries).into_iter().enumerate() {
        let artist = e.artist.trim().to_lowercase();
        let key = if artist.is_empty() {
            format!("\u{0}{i}")
        } else {
            artist
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(e),
…
```

### [PL-15] Retrait des doublons

Retire les entrées dont le fichier, ou le titre et l'artiste (majuscules ignorées), sont déjà présents plus haut ; seules les entrées conservées comptent comme déjà vues.

`app/src-tauri/src/playlist.rs` ligne 157

```rust
pub fn dedupe(&mut self) -> usize {
    let before = self.entries.len();
    let mut seen_paths = std::collections::HashSet::new();
    let mut seen_tracks = std::collections::HashSet::new();
    self.entries.retain(|e| {
        let title = e.title.trim().to_lowercase();
        let track = (!title.is_empty()).then(|| (title, e.artist.trim().to_lowercase()));
        let keep = !seen_paths.contains(&e.path)
            && track.as_ref().is_none_or(|t| !seen_tracks.contains(t));
        if keep {
            seen_paths.insert(e.path.clone());
            seen_tracks.extend(track);
…
```

### [PL-16] Entrée sauvegardée

Forme d'une entrée dans playlist.json : chemin, tags, durée, type et étiquette de grille.

`app/src-tauri/src/playlist.rs` ligne 385

```rust
#[derive(Serialize, Deserialize)]
struct SavedEntry {
    path: PathBuf,
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
    #[serde(default)]
    genre: String,
    #[serde(default)]
    year: Option<i32>,
…
```

### [PL-17] Playlist sauvegardée

Contenu de playlist.json : version du format, titres des platines en cours, entrées dans l'ordre.

`app/src-tauri/src/playlist.rs` ligne 410

```rust
#[derive(Serialize, Deserialize, Default)]
struct SavedQueue {
    version: u32,
    #[serde(default)]
    decks: Vec<PathBuf>,
    entries: Vec<SavedEntry>,
}
```

### [PL-18] Écriture de la playlist

Écrit playlist.json dans un fichier temporaire, force l'écriture sur disque puis le renomme.

`app/src-tauri/src/playlist.rs` ligne 419

```rust
pub fn save_queue(path: &Path, entries: &[Entry], decks: &[PathBuf]) -> Result<(), String> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let queue = SavedQueue {
        version: 1,
        decks: decks.to_vec(),
        entries: entries
            .iter()
            .map(|e| SavedEntry {
                path: e.path.clone(),
…
```

### [PL-19] Lecture de la playlist

Relit playlist.json sans relire les tags des fichiers.

`app/src-tauri/src/playlist.rs` ligne 455

```rust
pub fn load_queue(path: &Path) -> Result<(Vec<Entry>, Vec<PathBuf>), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let queue: SavedQueue =
        serde_json::from_reader(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    let entries = queue
        .entries
        .into_iter()
        .map(|s| Entry {
            id: 0,
            path: s.path,
            title: s.title,
            artist: s.artist,
…
```

### [PL-20] Fichier M3U

Reconnaît une playlist .m3u ou .m3u8 à son extension.

`app/src-tauri/src/playlist.rs` ligne 214

```rust
pub fn is_m3u(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8"))
}
```

### [PL-21] Lecture d'un M3U

Lit les chemins d'une playlist M3U (UTF-8 ou Latin-1, chemins relatifs au fichier, préfixe file://), en ignorant les commentaires et les fichiers non audio.

`app/src-tauri/src/playlist.rs` ligne 221

```rust
pub fn read_m3u(path: &Path) -> Vec<PathBuf> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    };
    let base = path.parent().unwrap_or(Path::new(""));
    text.lines()
        .map(str::trim)
…
```

### [PL-22] Écriture d'un M3U

Écrit la playlist en M3U étendu (#EXTINF : durée, artiste - titre), avec des fins de ligne Windows sous Windows.

`app/src-tauri/src/playlist.rs` ligne 248

```rust
pub fn write_m3u(path: &Path, entries: &[Entry]) -> Result<(), String> {
    let nl = if cfg!(windows) { "\r\n" } else { "\n" };
    let mut text = format!("#EXTM3U{nl}");
    for e in entries {
        let secs = e.duration.map_or(-1, |d| d.round() as i64);
        let label = if e.artist.trim().is_empty() {
            e.title.clone()
        } else {
            format!("{} - {}", e.artist, e.title)
        };
        text.push_str(&format!(
            "#EXTINF:{secs},{label}{nl}{}{nl}",
…
```


## THM

Thèmes de couleurs

### [THM-01] Thèmes intégrés

Black-Lab, Dark et Light, compilés dans l'application.

`app/src-tauri/src/themes.rs` ligne 5

```rust
const BUILTIN: &[(&str, &str)] = &[
    ("black-lab", include_str!("../../../themes/black-lab.toml")),
    ("dark", include_str!("../../../themes/dark.toml")),
    ("light", include_str!("../../../themes/light.toml")),
];
```

### [THM-02] Lecture d'un thème

Convertit un fichier TOML de thème en données pour l'interface.

`app/src-tauri/src/themes.rs` ligne 12

```rust
fn parse(id: &str, src: &str, builtin: bool) -> Option<Value> {
    let mut v: Value = toml::from_str(src).ok()?;
    let obj = v.as_object_mut()?;
    obj.insert("id".into(), Value::String(id.into()));
    obj.insert("builtin".into(), Value::Bool(builtin));
    Some(v)
}
```

### [THM-03] Liste des thèmes

Thèmes intégrés puis thèmes du dossier de l'utilisateur.

`app/src-tauri/src/themes.rs` ligne 21

```rust
pub fn list(user_dir: Option<&Path>) -> Vec<Value> {
    let mut out: Vec<Value> = BUILTIN
        .iter()
        .filter_map(|(id, src)| parse(id, src, true))
        .collect();
    if let Some(dir) = user_dir {
        if let Ok(read) = std::fs::read_dir(dir) {
            let mut files: Vec<_> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
            files.sort();
            for f in files {
                if f.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
…
```


## CTL

Contrôleur : fil central qui relie interface, moteur, diffusion, OBS et bibliothèque

### [CTL-01] Interface du contrôleur

Ce dont le contrôleur a besoin côté interface : émettre un événement et connaître le dossier des images. Remplaçable dans les tests.

`app/src-tauri/src/controller/mod.rs` ligne 51

```rust
pub trait Ui: Send + 'static {
    fn emit(&self, event: &'static str, payload: serde_json::Value);
    fn pictures_dir(&self) -> PathBuf;
}
```

### [CTL-02] Interface Tauri

Implémentation réelle : émet vers la fenêtre, journalise les échecs.

`app/src-tauri/src/controller/mod.rs` ligne 57

```rust
struct TauriUi(AppHandle);
```

### [CTL-03] Démarrage audio

Moteur démarré (ou erreur), nom de la sortie et fréquence.

`app/src-tauri/src/controller/mod.rs` ligne 75

```rust
pub struct Audio {
    engine: Option<Engine>,
    error: Option<String>,
    device: String,
    sample_rate: u32,
}
```

### [CTL-04] Ouverture de l'audio

Démarre le moteur sur la sortie choisie, se replie sur la sortie par défaut et journalise l'échec.

`app/src-tauri/src/controller/mod.rs` ligne 83

```rust
impl Audio {
    pub fn start(config: &AppConfig) -> Self {
        let wanted = config.audio.device.clone();
        let mic = voice::mic_input(&config.voice);
        let (engine, error) = match Engine::start_with(wanted.as_deref(), mic.clone()) {
            Ok(e) => (Some(e), None),
            Err(first) => match wanted {
                Some(_) => match Engine::start_with(None, mic) {
                    Ok(e) => (
                        Some(e),
                        Some(format!("{first} — sortie par défaut utilisée")),
                    ),
…
```

### [CTL-05] Travaux de fond

Résultats rendus par les fils de fond : scan, décodage d'un titre ou d'un pad, relevé d'auditeurs.

`app/src-tauri/src/controller/mod.rs` ligne 126

```rust
enum Work {
    Scanned {
        entries: Vec<Entry>,
        index: Option<usize>,
        to_deck: Option<Deck>,
    },
    Decoded {
        deck: Deck,
        job: u64,
        result: Result<Track, String>,
        play: bool,
    },
…
```

### [CTL-06] Heure en millisecondes

Horodatage Unix en millisecondes.

`app/src-tauri/src/controller/mod.rs` ligne 164

```rust
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
```

### [CTL-07] Heure en secondes

Horodatage Unix en secondes.

`app/src-tauri/src/controller/mod.rs` ligne 172

```rust
pub fn now_secs() -> i64 {
    (now_ms() / 1000) as i64
}
```

### [CTL-08] Lancement du contrôleur

Démarre le fil du contrôleur avec audio et bibliothèque, renvoie le canal de commandes.

`app/src-tauri/src/controller/mod.rs` ligne 177

```rust
pub fn spawn(
    app: AppHandle,
    secrets: Arc<Secrets>,
    config: AppConfig,
    warning: Option<String>,
    db_path: PathBuf,
) -> Result<Sender<UiCommand>, String> {
    let (tx, rx) = unbounded();
    thread::Builder::new()
        .name("blast-controller".into())
        .spawn(move || {
            let audio = Audio::start(&config);
…
```

### [CTL-09] Ouverture de la bibliothèque

Ouvre la base ; en cas d'échec, désactive historique et règles et prévient l'utilisateur.

`app/src-tauri/src/controller/mod.rs` ligne 210

```rust
fn open_library(
    db_path: &Path,
    memory_size: u32,
    warning: Option<String>,
) -> (Option<History>, Option<String>) {
    let opened = SqliteStore::open(db_path)
        .and_then(|store| History::new(Box::new(store), memory_size as usize));
    match opened {
        Ok(h) => (Some(h), warning),
        Err(e) => {
            journal::error(format!("base {} : {e}", db_path.display()));
            (
…
```

### [CTL-10] Contrôleur

État central : platines, playlist, diffusion, pads, micro, OBS et automation.

`app/src-tauri/src/controller/mod.rs` ligne 233

```rust
struct Controller {
    ui: Box<dyn Ui>,
    rx: Receiver<UiCommand>,
    work_tx: Sender<Work>,
    work_rx: Receiver<Work>,
    secrets: Arc<Secrets>,
    engine: Option<Engine>,
    audio_error: Option<String>,
    device: String,
    sample_rate: u32,
    playlist: Playlist,
    decks: [DeckSide; 2],
…
```

### [CTL-11] Initialisation

Applique les réglages au moteur, connecte OBS si demandé, charge les pads.

`app/src-tauri/src/controller/mod.rs` ligne 268

```rust
fn new(
    ui: Box<dyn Ui>,
    rx: Receiver<UiCommand>,
    secrets: Arc<Secrets>,
    config: AppConfig,
    startup_warning: Option<String>,
    audio: Audio,
    library: Option<History>,
) -> Self {
    let (work_tx, work_rx) = unbounded();
    let mic = MicState::from_engine(audio.engine.as_ref());
    let mut c = Self {
…
```

### [CTL-12] Boucle principale

Toutes les 15 ms : commandes de l'interface, travaux de fond, événements du moteur, prise antenne, OBS, sorties, statistiques et entretien.

`app/src-tauri/src/controller/mod.rs` ligne 326

```rust
fn run(mut self) {
    loop {
        match self.rx.recv_timeout(Duration::from_millis(15)) {
            Ok(cmd) => {
                self.handle(cmd);
                while let Ok(cmd) = self.rx.try_recv() {
                    self.handle(cmd);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
…
```

### [CTL-13] Réception des travaux

Applique les résultats des fils de fond.

`app/src-tauri/src/controller/mod.rs` ligne 352

```rust
fn drain_work(&mut self) {
    while let Ok(w) = self.work_rx.try_recv() {
        self.on_work(w);
    }
}
```

### [CTL-14] Événements du moteur

Lit jusqu'à 64 événements par tour et ne garde que le dernier tick.

`app/src-tauri/src/controller/mod.rs` ligne 359

```rust
fn pump_engine(&mut self) {
    let mut events = Vec::new();
    if let Some(engine) = &self.engine {
        let mut tick = None;
        for _ in 0..64 {
            match engine.try_event() {
                Some(e @ Event::Tick { .. }) => tick = Some(e),
                Some(e) => events.push(e),
                None => break,
            }
        }
        events.extend(tick);
…
```

### [CTL-15] Envoi au moteur

Transmet une commande au mixeur si le moteur tourne.

`app/src-tauri/src/controller/mod.rs` ligne 379

```rust
fn send(&self, cmd: Command) {
    if let Some(e) = &self.engine {
        e.send(cmd);
    }
}
```

### [CTL-16] Émission

Sérialise une vue et l'envoie à l'interface.

`app/src-tauri/src/controller/mod.rs` ligne 386

```rust
fn emit<T: Serialize>(&self, event: &'static str, payload: &T) {
    match serde_json::to_value(payload) {
        Ok(v) => self.ui.emit(event, v),
        Err(e) => journal::error(format!("sérialisation {event} : {e}")),
    }
}
```

### [CTL-17] Notification

Affiche un message à l'utilisateur et journalise avertissements et erreurs.

`app/src-tauri/src/controller/mod.rs` ligne 394

```rust
fn notice(&self, level: &'static str, message: impl Into<String>) {
    let message = message.into();
    if level != "info" {
        journal::log(level, &message);
    }
    self.emit(EV_NOTICE, &Notice { level, message });
}
```

### [CTL-18] Aiguillage des commandes

Dirige chaque commande de l'interface vers son traitement, puis publie l'état.

`app/src-tauri/src/controller/mod.rs` ligne 403

```rust
fn handle(&mut self, cmd: UiCommand) {
    match cmd {
        UiCommand::Refresh => self.refresh(),
        UiCommand::Shutdown { done } => {
            self.flush_queue();
            let _ = done.send(());
        }
        UiCommand::CastStart => self.start_casting(),
        UiCommand::CastStop => self.stop_casting(),
        UiCommand::Next { deck } => self.next(deck),
        UiCommand::MicOpen { open } => self.mic_open(open),
        UiCommand::PadPlay { index } => self.pad_play(index),
…
```

### [CTL-19] Rafraîchissement

Republie toutes les vues et l'avertissement de démarrage éventuel.

`app/src-tauri/src/controller/mod.rs` ligne 470

```rust
fn refresh(&mut self) {
    self.emit_playlist();
    self.emit_cast();
    self.emit_pads();
    self.emit_jingles();
    self.emit_library();
    self.emit_obs();
    self.emit_history();
    self.emit_chat_state();
    self.emit_chat_history();
    if let Some(w) = self.startup_warning.take() {
        self.notice("warn", w);
…
```

### [CTL-20] Nouveaux réglages

Applique traitement antenne, automation, OBS, voix, pads et sorties modifiés.

`app/src-tauri/src/controller/mod.rs` ligne 486

```rust
fn apply_config(&mut self, cfg: AppConfig) {
    if cfg.audio.master_preset != self.master_preset {
        self.master_preset = cfg.audio.master_preset.clone();
        self.send(Command::SetMasterParams(master_preset(&self.master_preset)));
    }
    self.apply_automation(cfg.automation.clone());
    self.apply_aura(cfg.aura.clone());
    self.apply_voice(&cfg.voice);
    self.sync_pads(&cfg.soundboard);
    self.sync_jingles(&cfg.jingles);
    if cfg.categories != self.categories {
        self.categories = cfg.categories;
…
```

### [CTL-21] Aiguillage des travaux

Dirige chaque résultat de fond vers son traitement.

`app/src-tauri/src/controller/mod.rs` ligne 508

```rust
fn on_work(&mut self, w: Work) {
    match w {
        Work::Scanned {
            entries,
            index,
            to_deck,
        } => self.on_scanned(entries, index, to_deck),
        Work::Listeners { id, result } => self.on_listeners(id, result),
        Work::PadDecoded {
            index,
            path,
            result,
…
```

### [CTL-22] Événements du mixeur

Met à jour platines, vumètres et micro à chaque tick ; gère fin de titre, fondus et fin des sons de pads.

`app/src-tauri/src/controller/mod.rs` ligne 543

```rust
fn on_event(&mut self, e: Event) {
    match e {
        Event::Tick {
            decks,
            crossfader,
            peak,
            sfx,
            mic_peak,
            mic_open,
            duck,
            reduction,
        } => {
…
```

### [CTL-23] Platine à l'antenne

Platine désignée par la position du crossfader.

`app/src-tauri/src/controller/mod.rs` ligne 587

```rust
fn on_air_side(&self) -> usize {
    if self.crossfader < 0.5 {
        0
    } else {
        1
    }
}
```

### [CTL-24] Platine côté interface

A ou B, tel que l'envoie l'interface.

`app/src-tauri/src/controller/commands.rs` ligne 9

```rust
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum Deck {
    A,
    B,
}
```

### [CTL-25] Actions OBS de l'interface

Connexion, scène, stream, enregistrement, replay, capture, volume et muet.

`app/src-tauri/src/controller/commands.rs` ligne 44

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ObsAction {
    Connect,
    Disconnect,
    Scene { name: String },
    Stream,
    Record,
    RecordPause,
    Replay,
    Snap,
    Volume { input: String, mul: f32 },
…
```

### [CTL-26] Commandes de l'interface

Tout ce que l'interface peut demander au contrôleur ; les réglages passent par la variante Config.

`app/src-tauri/src/controller/commands.rs` ligne 60

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiCommand {
    Refresh,
    #[serde(skip)]
    Shutdown {
        done: crossbeam_channel::Sender<()>,
    },
    AddPaths {
        paths: Vec<PathBuf>,
        index: Option<usize>,
    },
…
```

### [CTL-27] Noms des événements

Canaux blast:// écoutés par l'interface : état, playlist, notifications, diffusion, pads, OBS, historique.

`app/src-tauri/src/controller/views.rs` ligne 11

```rust
pub const EV_STATE: &str = "blast://state";
```

### [CTL-28] Vue d'une notification

Niveau et message.

`app/src-tauri/src/controller/views.rs` ligne 26

```rust
#[derive(Serialize, Clone)]
pub struct Notice {
    pub level: &'static str,
    pub message: String,
}
```

### [CTL-29] Vue d'une source OBS

Nom, type, muet et volume.

`app/src-tauri/src/controller/views.rs` ligne 33

```rust
#[derive(Serialize, Clone)]
pub struct ObsInputView {
    pub name: String,
    pub kind: String,
    pub muted: bool,
    pub volume: f32,
}
```

### [CTL-30] Vue OBS

État de connexion, scènes, stream, enregistrement et sources audio.

`app/src-tauri/src/controller/views.rs` ligne 42

```rust
#[derive(Serialize, Clone, Default)]
pub struct ObsView {
    pub status: &'static str,
    pub detail: String,
    pub obs_version: String,
    pub scenes: Vec<String>,
    pub current: Option<String>,
    pub stream: bool,
    pub stream_since: Option<u64>,
    pub record: bool,
    pub record_paused: bool,
    pub record_since: Option<u64>,
…
```

### [CTL-31] Vue d'un pad

Nom, couleur, fichier, prêt ou en erreur.

`app/src-tauri/src/controller/views.rs` ligne 69

```rust
#[derive(Serialize, Clone)]
pub struct PadView {
    pub index: u16,
    pub name: String,
    pub color: String,
    pub path: PathBuf,
    pub ready: bool,
    pub error: Option<String>,
}
```

### [CTL-32] Vue de la Sound FX

Pads affectés et nombre total.

`app/src-tauri/src/controller/views.rs` ligne 80

```rust
#[derive(Serialize, Clone)]
pub struct PadsView {
    pub pads: Vec<PadView>,
    pub count: u16,
}
```

### [CTL-33] Vue du micro

Disponibilité, ouverture, niveau, nom et erreur.

`app/src-tauri/src/controller/views.rs` ligne 87

```rust
#[derive(Serialize, Clone)]
pub struct MicView {
    pub available: bool,
    pub open: bool,
    pub peak: f32,
    pub name: Option<String>,
    pub error: Option<String>,
}
```

### [CTL-34] Vue d'une platine

État, position, durée, titre, artiste et fichier.

`app/src-tauri/src/controller/views.rs` ligne 97

```rust
#[derive(Serialize, Clone)]
pub struct DeckView {
    pub state: &'static str,
    pub position: f64,
    pub duration: f64,
    pub title: String,
    pub artist: String,
    pub path: Option<PathBuf>,
}
```

### [CTL-35] Vue d'état

Platines, crossfader, vumètres, auto-mix, platine à l'antenne, sortie audio, micro, pads en cours, ducking et réduction.

`app/src-tauri/src/controller/views.rs` ligne 108

```rust

```

### [CTL-36] Vue de playlist

Entrées avec motif d'attente et durée totale.

`app/src-tauri/src/controller/views.rs` ligne 151

```rust
#[derive(Serialize, Clone)]
pub struct PlaylistView<'a> {
    pub entries: &'a [Entry],
    pub total: f64,
}
```

### [CTL-37] Vue d'une sortie

État, cible, auditeurs, pic et heure de passage en direct.

`app/src-tauri/src/controller/views.rs` ligne 158

```rust
#[derive(Serialize, Clone)]
pub struct CastOutputView {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub target: String,
    pub enabled: bool,
    pub status: OutputStatus,
    pub listeners: Option<u32>,
    pub peak: Option<u32>,
    pub live_since: Option<u64>,
}
```

### [CTL-38] Vue de diffusion

Diffusion en cours, titre, sorties, auditeurs, pic de session et début.

`app/src-tauri/src/controller/views.rs` ligne 172

```rust
#[derive(Serialize, Clone)]
pub struct CastView {
    pub casting: bool,
    pub song: String,
    pub outputs: Vec<CastOutputView>,
    pub listeners: Option<u32>,
    pub session_peak: u32,
    pub since: Option<u64>,
}
```

### [CTL-39] Vue d'historique

Tampons mémoire envoyés à l'interface : flux global, morceaux, jingles et pubs, avec statistiques, passage en cours et taille des tampons.

`app/src-tauri/src/controller/views.rs` ligne 183

```rust
#[derive(Serialize, Clone, Default)]
pub struct HistoryView {
    pub all: Vec<HistoryRow>,
    pub morceau: Vec<HistoryRow>,
    pub jingle: Vec<HistoryRow>,
    #[serde(rename = "pub")]
    pub pub_: Vec<HistoryRow>,
    pub stats: Option<LibraryStats>,
    pub current: Option<i64>,
    pub capacity: u32,
}
```

### [CTL-40] Platine vide

État publié d'une platine sans titre.

`app/src-tauri/src/controller/decks.rs` ligne 13

```rust
pub const EMPTY_STATUS: DeckStatus = DeckStatus {
    state: DeckState::Empty,
    position: 0,
    frames: 0,
};
```

### [CTL-41] Titre d'une platine

Titre, artiste, fichier, chargement en cours et fin de lecture.

`app/src-tauri/src/controller/decks.rs` ligne 20

```rust
#[derive(Debug, Default, Clone)]
pub struct DeckMeta {
    pub title: String,
    pub artist: String,
    pub path: Option<PathBuf>,
    pub loading: Option<u64>,
    pub finished: bool,
    pub kind: Kind,
}
```

### [CTL-42] Platine

Titre et dernier état publié par le mixeur.

`app/src-tauri/src/controller/decks.rs` ligne 31

```rust
#[derive(Debug, Clone)]
pub struct DeckSide {
    pub meta: DeckMeta,
    pub status: DeckStatus,
}
```

### [CTL-43] État d'une platine

En lecture, prête, active (chargement, lecture ou en attente), libellé et remise à zéro.

`app/src-tauri/src/controller/decks.rs` ligne 47

```rust
impl DeckSide {
    pub fn is_playing(&self) -> bool {
        self.status.state == DeckState::Playing && !self.meta.finished
    }

    pub fn is_cued(&self) -> bool {
        self.meta.path.is_some() && !self.meta.finished
    }

    pub fn is_active(&self) -> bool {
        self.meta.loading.is_some() || self.is_playing() || self.is_cued()
    }
…
```

### [CTL-44] Chargement d'un titre

Décode un titre en arrière-plan ; seul le dernier chargement demandé est retenu.

`app/src-tauri/src/controller/decks.rs` ligne 88

```rust
pub(super) fn request_load(&mut self, deck: Deck, entry: Entry, play: bool) {
    self.next_job += 1;
    let job = self.next_job;
    self.decks[deck.index()].meta = DeckMeta {
        title: entry.title.clone(),
        artist: entry.artist.clone(),
        path: Some(entry.path.clone()),
        loading: Some(job),
        finished: false,
        kind: entry.kind,
    };
    let tx = self.work_tx.clone();
…
```

### [CTL-45] Titre décodé

Charge le titre dans le mixeur, le lance si demandé, déclenche un fondu en attente ou signale l'erreur.

`app/src-tauri/src/controller/decks.rs` ligne 113

```rust
pub(super) fn on_decoded(
    &mut self,
    deck: Deck,
    job: u64,
    result: Result<Track, String>,
    play: bool,
) {
    let i = deck.index();
    if self.decks[i].meta.loading != Some(job) {
        return;
    }
    self.decks[i].meta.loading = None;
…
```

### [CTL-46] Fin de titre

Marque la platine comme terminée.

`app/src-tauri/src/controller/decks.rs` ligne 168

```rust
pub(super) fn on_ended(&mut self, i: usize) {
    let side = &mut self.decks[i];
    side.meta.finished = true;
    side.status.state = DeckState::Finished;
    side.status.position = side.status.frames;
}
```

### [CTL-47] Retour au début

Remet au début une platine terminée avant de la relancer.

`app/src-tauri/src/controller/decks.rs` ligne 176

```rust
fn rewind_if_finished(&mut self, deck: Deck) {
    let side = &mut self.decks[deck.index()];
    if side.meta.finished {
        side.meta.finished = false;
        self.send(Command::Stop(deck.id()));
    }
}
```

### [CTL-48] Fondu

Lance la platine cible et le fondu du crossfader vers elle.

`app/src-tauri/src/controller/decks.rs` ligne 185

```rust
pub(super) fn crossfade_to(&mut self, to: Deck) {
    if !self.decks[to.index()].is_ready() {
        return;
    }
    self.rewind_if_finished(to);
    self.send(Command::Play(to.id()));
    self.send(Command::StartCrossfade {
        to: to.id(),
        frames: (self.fade.max(0.1) * self.sample_rate as f32) as usize,
    });
}
```

### [CTL-49] Fondu ou chargement

Fait le fondu si la platine cible a un titre, sinon charge le suivant de la playlist puis fait le fondu.

`app/src-tauri/src/controller/decks.rs` ligne 198

```rust
pub(super) fn fade_or_fetch(&mut self, to: Deck) {
    let side = &self.decks[to.index()];
    if side.meta.loading.is_some() {
        self.pending_fade = Some(to);
    } else if side.is_playing() || side.is_cued() {
        self.crossfade_to(to);
    } else if let Some(e) = self.take_next_auto() {
        self.request_load(to, e, false);
        self.pending_fade = Some(to);
        self.emit_playlist();
    } else {
        self.notice("warn", "Plus aucun titre dans la playlist.");
…
```

### [CTL-50] Bouton suivant

Sur une platine en lecture : fondu vers l'autre. Sur une platine arrêtée : charge le titre suivant.

`app/src-tauri/src/controller/decks.rs` ligne 214

```rust
pub(super) fn next(&mut self, deck: Deck) {
    let i = deck.index();
    if self.is_playing(i) {
        self.fade_or_fetch(Deck::from_index(1 - i));
    } else if self.decks[i].meta.loading.is_none() {
        match self.take_next_auto() {
            Some(e) => {
                self.request_load(deck, e, false);
                self.emit_playlist();
            }
            None => self.notice("warn", "Plus aucun titre dans la playlist."),
        }
…
```

### [CTL-51] Retour en playlist

Remet en tête de playlist un titre chargé mais jamais lu qu'on remplace.

`app/src-tauri/src/controller/decks.rs` ligne 230

```rust
pub(super) fn return_cued(&mut self, deck: Deck) {
    let side = &self.decks[deck.index()];
    if side.meta.loading.is_some() || side.meta.finished || side.status.position != 0 {
        return;
    }
    if let Some(path) = side.meta.path.clone() {
        self.playlist
            .insert(vec![playlist::make_entry(&path, &self.categories)], Some(0));
    }
}
```

### [CTL-52] Auto-mix

Garde les deux platines garnies : démarre la première, prépare la suivante.

`app/src-tauri/src/controller/decks.rs` ligne 242

```rust
pub(super) fn fill_automix(&mut self) {
    if !self.automix {
        return;
    }
    let mut changed = self.rotation_fill();
    if !self.decks[0].is_active() && !self.decks[1].is_active() {
        if let Some(e) = self.take_next_auto() {
            let start = Deck::from_index(self.on_air_side());
            self.request_load(start, e, true);
            changed = true;
        }
    }
…
```

### [CTL-53] Chargement depuis la playlist

Charge une entrée sur une platine, sauf si celle-ci est à l'antenne ou le fichier illisible.

`app/src-tauri/src/controller/decks.rs` ligne 269

```rust
pub(super) fn load_entry(&mut self, id: u64, deck: Deck) {
    if self.is_playing(deck.index()) {
        self.notice(
            "warn",
            format!(
                "La platine {} est à l'antenne : arrête-la avant d'y charger un titre.",
                deck.label()
            ),
        );
    } else if let Some(bad) = self
        .playlist
        .entries()
…
```

### [CTL-54] Lecture

Lance une platine prête.

`app/src-tauri/src/controller/decks.rs` ligne 293

```rust
pub(super) fn play(&mut self, deck: Deck) {
    if self.decks[deck.index()].is_ready() {
        self.rewind_if_finished(deck);
        self.send(Command::Play(deck.id()));
    }
}
```

### [CTL-55] Arrêt

Arrête une platine et la remet au début.

`app/src-tauri/src/controller/decks.rs` ligne 301

```rust
pub(super) fn stop(&mut self, deck: Deck) {
    self.send(Command::Stop(deck.id()));
    self.decks[deck.index()].meta.finished = false;
}
```

### [CTL-56] Éjection

Vide une platine qui n'est pas en lecture.

`app/src-tauri/src/controller/decks.rs` ligne 307

```rust
pub(super) fn eject(&mut self, deck: Deck) {
    if !self.is_playing(deck.index()) {
        self.send(Command::Eject(deck.id()));
        self.decks[deck.index()].reset();
        self.fill_automix();
    }
}
```

### [CTL-57] Déplacement

Place la lecture à une fraction du titre.

`app/src-tauri/src/controller/decks.rs` ligne 316

```rust
pub(super) fn seek(&mut self, deck: Deck, fraction: f32) {
    let side = &mut self.decks[deck.index()];
    let frames = side.status.frames;
    if frames == 0 {
        return;
    }
    let target = (fraction.clamp(0.0, 1.0) as f64 * frames as f64) as usize;
    side.meta.finished = false;
    self.send(Command::Seek(
        deck.id(),
        target.min(frames.saturating_sub(1)),
    ));
…
```

### [CTL-58] Réglage de l'auto-mix

Transmet l'activation et la durée de fondu au mixeur.

`app/src-tauri/src/controller/decks.rs` ligne 331

```rust
pub(super) fn send_automix(&self) {
    self.send(Command::SetAutoMix {
        enabled: self.automix,
        fade_frames: (self.fade * self.sample_rate as f32) as usize,
    });
}
```

### [CTL-59] Vue d'une platine

Convertit trames en secondes pour l'interface.

`app/src-tauri/src/controller/decks.rs` ligne 339

```rust
fn deck_view(&self, i: usize) -> DeckView {
    let side = &self.decks[i];
    let rate = self.sample_rate as f64;
    DeckView {
        state: side.state_label(),
        position: side.status.position as f64 / rate,
        duration: side.status.frames as f64 / rate,
        title: side.meta.title.clone(),
        artist: side.meta.artist.clone(),
        path: side.meta.path.clone(),
    }
}
```

### [CTL-60] Publication de l'état

Envoie la vue d'état à l'interface.

`app/src-tauri/src/controller/decks.rs` ligne 353

```rust
pub(super) fn emit_state(&self) {
    let side = self.on_air_side();
    let view = StateView {
        decks: [self.deck_view(0), self.deck_view(1)],
        crossfader: self.crossfader,
        peak: self.peak,
        automix: self.automix,
        fade: self.fade,
        on_air: self.is_playing(side).then(|| Deck::from_index(side)),
        device: self.device.clone(),
        sample_rate: self.sample_rate,
        audio_error: self.audio_error.clone(),
…
```

### [CTL-61] Fréquence de purge

Purge de l'historique au plus une fois par heure.

`app/src-tauri/src/controller/automation.rs` ligne 13

```rust
const PURGE_EVERY: Duration = Duration::from_secs(3600);
```

### [CTL-62] Rafraîchissement des règles

Toutes les 30 s, seuls les motifs d'attente sont republiés (événement blast://blocked).

`app/src-tauri/src/controller/automation.rs` ligne 15

```rust
const RULES_REFRESH_EVERY: Duration = Duration::from_secs(30);
```

### [CTL-63] Taille de l'historique affiché *(retiré)*

Nombre de passages envoyés à l'interface ; remplacé par la taille des tampons mémoire [CFG-06].

### [CTL-64] Passage en cours

Platine, fichier, ligne d'historique et heure de début.

`app/src-tauri/src/controller/automation.rs` ligne 18

```rust
pub struct CurrentPlay {
    pub deck: usize,
    pub path: PathBuf,
    pub hist_id: Option<i64>,
    pub started: Instant,
}
```

### [CTL-65] État d'automation

Historique (base et tampons mémoire), réglages, passage en cours et horloges d'entretien.

`app/src-tauri/src/controller/automation.rs` ligne 26

```rust
pub struct AutoState {
    pub history: Option<History>,
    pub settings: AutomationSettings,
    pub current: Option<CurrentPlay>,
    pub last_purge: Option<Instant>,
    pub last_rules_refresh: Instant,
}
```

### [CTL-66] Choix du titre suivant

Titre conforme, titre forcé faute de mieux, ou playlist vide.

`app/src-tauri/src/controller/automation.rs` ligne 51

```rust
pub enum NextPick {
    Clean(u64),
    Forced { id: u64, title: String, why: String },
    Empty,
}
```

### [CTL-67] Sélection du titre suivant

Premier titre lisible conforme aux règles ; sinon le premier titre lisible, avec le motif.

`app/src-tauri/src/controller/automation.rs` ligne 58

```rust
pub fn pick_next<'a>(
    entries: impl IntoIterator<Item = &'a Entry>,
    mut violation: impl FnMut(&Entry) -> Option<String>,
) -> NextPick {
    let mut fallback = None;
    for e in entries.into_iter().filter(|e| e.error.is_none()) {
        match violation(e) {
            None => return NextPick::Clean(e.id),
            Some(why) => {
                fallback.get_or_insert_with(|| (e.id, e.title.clone(), why));
            }
        }
…
```

### [CTL-68] Motif d'attente

Même artiste déjà sur une platine, ou règle enfreinte selon l'historique ; jingles, pubs et titres d'émission en sont exemptés.

`app/src-tauri/src/controller/automation.rs` ligne 79

```rust
pub(super) fn violation_for(&self, e: &Entry, now: i64) -> Option<String> {
    if e.error.is_some() || e.slot.as_ref().is_some_and(|s| s.fixed) {
        return None;
    }
    self.violation_of(&e.path.to_string_lossy(), &e.title, &e.artist, e.kind, now)
}
```

### [CTL-69] Titre suivant

Retire de la playlist le titre choisi et prévient si les règles n'ont pas pu être respectées.

`app/src-tauri/src/controller/automation.rs` ligne 126

```rust
pub(super) fn take_next_auto(&mut self) -> Option<Entry> {
    let now = now_secs();
    let pick = pick_next(self.playlist.entries(), |e| self.violation_for(e, now));
    let id = match pick {
        NextPick::Clean(id) => id,
        NextPick::Forced { id, title, why } => {
            self.notice(
                "warn",
                format!("Aucun titre ne respecte les règles de séparation : « {title} » passe quand même ({why})."),
            );
            id
        }
…
```

### [CTL-70] Suivi des passages

Ouvre un passage typé quand un titre passe à l'antenne, le clôt avec sa durée quand il en sort.

`app/src-tauri/src/controller/automation.rs` ligne 144

```rust
pub(super) fn track_history(&mut self) {
    let side = self.on_air_side();
    let d = &self.decks[side];
    let on_air =
        matches!(d.status.state, DeckState::Playing | DeckState::Paused) && !d.meta.finished;
    let now_path = if on_air { d.meta.path.clone() } else { None };
    let unchanged = matches!(
        (&self.auto.current, &now_path),
        (Some(cp), Some(p)) if cp.deck == side && &cp.path == p
    );
    if unchanged {
        return;
…
```

### [CTL-71] Effacement de l'historique

Vide l'historique et détache le passage en cours.

`app/src-tauri/src/controller/automation.rs` ligne 215

```rust
pub(super) fn history_clear(&mut self) {
    if let Some(history) = self.auto.history.as_mut() {
        if let Err(e) = history.clear() {
            self.notice("error", format!("Impossible de vider l'historique : {e}"));
        }
    }
    if let Some(cp) = self.auto.current.as_mut() {
        cp.hist_id = None;
    }
    self.emit_history();
    self.emit_playlist();
}
```

### [CTL-72] Nouveaux réglages d'automation

Applique les règles, redimensionne les tampons mémoire et relance la purge.

`app/src-tauri/src/controller/automation.rs` ligne 229

```rust
pub(super) fn apply_automation(&mut self, settings: AutomationSettings) {
    if settings != self.auto.settings {
        if let Some(history) = self.auto.history.as_mut() {
            if let Err(e) = history.set_capacity(settings.memory_size as usize) {
                journal::warn(format!("historique (rechargement) : {e}"));
            }
        }
        self.auto.settings = settings;
        self.auto.last_purge = None;
        self.emit_playlist();
    }
}
```

### [CTL-73] Entretien

Purge horaire et rafraîchissement des motifs d'attente.

`app/src-tauri/src/controller/automation.rs` ligne 243

```rust
pub(super) fn housekeeping(&mut self) {
    let purge_due = self
        .auto
        .last_purge
        .is_none_or(|t| t.elapsed() > PURGE_EVERY);
    if purge_due {
        self.auto.last_purge = Some(Instant::now());
        self.purge_history();
    }
    if self.auto.last_rules_refresh.elapsed() > RULES_REFRESH_EVERY {
        self.auto.last_rules_refresh = Instant::now();
        self.emit_blocked();
…
```

### [CTL-74] Purge de l'historique

Supprime les passages plus anciens que la durée de conservation.

`app/src-tauri/src/controller/automation.rs` ligne 259

```rust
fn purge_history(&mut self) {
    let days = self.auto.settings.history_days;
    let Some(history) = self.auto.history.as_mut().filter(|_| days > 0) else {
        return;
    };
    let cutoff = now_secs() - days as i64 * 86_400;
    match history.purge_older_than(cutoff) {
        Ok(0) => {}
        Ok(n) => {
            journal::info(format!(
                "historique : {n} passage(s) de plus de {days} jours purgé(s)"
            ));
…
```

### [CTL-75] Publication de l'historique

Envoie les tampons mémoire (global et par catégorie) sans interroger la base.

`app/src-tauri/src/controller/automation.rs` ligne 278

```rust
pub(super) fn emit_history(&self) {
    let view = match &self.auto.history {
        Some(h) => {
            let rows = |k| h.rows(k).iter().cloned().collect();
            HistoryView {
                all: rows(None),
                morceau: rows(Some(Kind::Morceau)),
                jingle: rows(Some(Kind::Jingle)),
                pub_: rows(Some(Kind::Pub)),
                stats: h.stats().ok(),
                current: self.auto.current.as_ref().and_then(|c| c.hist_id),
                capacity: self.auto.settings.memory_size,
…
```

### [CTL-76] Fréquence des statistiques

Relevé des auditeurs toutes les 10 s.

`app/src-tauri/src/controller/cast.rs` ligne 12

```rust
const STATS_EVERY: Duration = Duration::from_secs(10);
```

### [CTL-77] État de diffusion

Sorties, fils d'envoi, états, statistiques, titre en cours et prise antenne.

`app/src-tauri/src/controller/cast.rs` ligne 15

```rust
pub struct CastState {
    pub outputs: Vec<OutputConfig>,
    pub caster: Caster,
    pub casting: bool,
    pub status: HashMap<String, OutputStatus>,
    pub stats: HashMap<String, (Option<Listeners>, u32)>,
    pub live_since: HashMap<String, u64>,
    pub session_peak: u32,
    pub last_stats: Instant,
    pub master: Vec<f32>,
    pub song: String,
}
```

### [CTL-78] Calculs de diffusion

Total des auditeurs en direct et vues des sorties.

`app/src-tauri/src/controller/cast.rs` ligne 29

```rust
impl CastState {
    pub fn new(outputs: Vec<OutputConfig>, sample_rate: u32) -> Self {
        Self {
            outputs,
            caster: Caster::new(sample_rate),
            casting: false,
            status: HashMap::new(),
            stats: HashMap::new(),
            live_since: HashMap::new(),
            session_peak: 0,
            last_stats: Instant::now(),
            master: Vec::with_capacity(sample_rate as usize),
…
```

### [CTL-79] Envoi du son

Transmet la prise antenne aux fils d'envoi pendant la diffusion.

`app/src-tauri/src/controller/cast.rs` ligne 91

```rust
pub(super) fn pump_master(&mut self) {
    let Some(engine) = self.engine.as_mut() else {
        return;
    };
    let cast = &mut self.cast;
    cast.master.clear();
    engine.read_master(&mut cast.master);
    if cast.casting && cast.caster.is_active() && !cast.master.is_empty() {
        cast.caster.push(Arc::from(&cast.master[..]));
    }
}
```

### [CTL-80] États des sorties

Relève les changements d'état et l'heure de passage en direct.

`app/src-tauri/src/controller/cast.rs` ligne 104

```rust
pub(super) fn pump_caster(&mut self) {
    let updates = self.cast.caster.poll();
    if updates.is_empty() {
        return;
    }
    for u in updates {
        if u.status == OutputStatus::Live {
            self.cast
                .live_since
                .entry(u.id.clone())
                .or_insert_with(now_ms);
            self.cast.last_stats = Instant::now() - Duration::from_secs(8);
…
```

### [CTL-81] Titre à l'antenne

« Artiste - Titre » de la platine à l'antenne.

`app/src-tauri/src/controller/cast.rs` ligne 128

```rust
pub(super) fn now_playing(&self) -> String {
    let side = self.on_air_side();
    if !self.is_playing(side) {
        return String::new();
    }
    let m = &self.decks[side].meta;
    if m.kind != blast_library::Kind::Morceau {
        return String::new();
    }
    match (m.artist.trim(), m.title.trim()) {
        ("", t) => t.to_string(),
        (a, t) => format!("{a} - {t}"),
…
```

### [CTL-82] Changement de titre

Met à jour le titre envoyé aux serveurs et à OBS.

`app/src-tauri/src/controller/cast.rs` ligne 144

```rust
pub(super) fn update_song(&mut self) {
    let song = self.now_playing();
    if !song.is_empty() && song != self.cast.song {
        self.cast.song = song;
        self.cast.caster.set_metadata(&self.cast.song);
        self.obs_now_playing();
        self.emit_cast();
        let song = self.cast.song.clone();
        self.chat_announce(&song);
    }
}
```

### [CTL-83] Début de diffusion

Démarre chaque sortie active qui a un mot de passe.

`app/src-tauri/src/controller/cast.rs` ligne 157

```rust
pub(super) fn start_casting(&mut self) {
    self.cast.session_peak = 0;
    let enabled: Vec<OutputConfig> = self
        .cast
        .outputs
        .iter()
        .filter(|o| o.enabled)
        .cloned()
        .collect();
    if enabled.is_empty() {
        self.notice(
            "warn",
…
```

### [CTL-84] Fin de diffusion

Arrête toutes les sorties.

`app/src-tauri/src/controller/cast.rs` ligne 203

```rust
pub(super) fn stop_casting(&mut self) {
    let cast = &mut self.cast;
    cast.casting = false;
    cast.live_since.clear();
    cast.stats.clear();
    cast.caster.stop_all();
    for s in cast.status.values_mut() {
        *s = OutputStatus::Off;
    }
    self.emit_cast();
}
```

### [CTL-85] Nouvelles sorties

Applique les sorties modifiées ; relance la diffusion en cours, y compris après une erreur.

`app/src-tauri/src/controller/cast.rs` ligne 216

```rust
pub(super) fn apply_outputs(&mut self, outputs: Vec<OutputConfig>) {
    let errored = self
        .cast
        .status
        .values()
        .any(|s| matches!(s, OutputStatus::Error(_)));
    if outputs == self.cast.outputs && !(self.cast.casting && errored) {
        return;
    }
    let cast = &mut self.cast;
    cast.outputs = outputs;
    let ids: Vec<String> = cast.outputs.iter().map(|o| o.id.clone()).collect();
…
```

### [CTL-86] Relevé des auditeurs

Interroge les serveurs en direct en arrière-plan.

`app/src-tauri/src/controller/cast.rs` ligne 239

```rust
pub(super) fn poll_listeners(&mut self) {
    if !self.cast.casting || self.cast.last_stats.elapsed() < STATS_EVERY {
        return;
    }
    self.cast.last_stats = Instant::now();
    for o in self
        .cast
        .outputs
        .iter()
        .filter(|o| self.cast.is_live(&o.id))
    {
        let cfg = o.clone();
…
```

### [CTL-87] Auditeurs reçus

Met à jour compteurs et pics ; les échecs vont au journal.

`app/src-tauri/src/controller/cast.rs` ligne 260

```rust
pub(super) fn on_listeners(&mut self, id: String, result: Result<Listeners, String>) {
    let l = match result {
        Ok(l) => l,
        Err(e) => {
            crate::journal::warn(format!("statistiques {id} : {e}"));
            return;
        }
    };
    let entry = self.cast.stats.entry(id).or_insert((None, 0));
    entry.0 = Some(l);
    entry.1 = entry.1.max(l.current);
    if let Some(total) = self.cast.total_listeners() {
…
```

### [CTL-88] Publication de la diffusion

Envoie la vue de diffusion.

`app/src-tauri/src/controller/cast.rs` ligne 278

```rust
pub(super) fn emit_cast(&self) {
    self.emit(EV_CAST, &self.cast.view());
}
```

### [CTL-89] État OBS

Client, réglages et vue.

`app/src-tauri/src/controller/obs.rs` ligne 8

```rust
pub struct ObsState {
    pub client: Option<AuraClient>,
    pub settings: AuraSettings,
    pub view: ObsView,
}
```

### [CTL-90] Mise à jour de l'état OBS

Transforme les événements OBS en vue et en messages.

`app/src-tauri/src/controller/obs.rs` ligne 15

```rust
impl ObsState {
    pub fn new(settings: AuraSettings) -> Self {
        Self {
            client: None,
            view: ObsView::idle("off", settings.now_playing_source.clone()),
            settings,
        }
    }

    pub fn connected(&self) -> bool {
        self.view.status == "connected"
    }
…
```

### [CTL-91] Connexion à OBS

Démarre le client OBS avec le mot de passe du trousseau.

`app/src-tauri/src/controller/obs.rs` ligne 108

```rust
pub(super) fn obs_connect(&mut self) {
    self.obs.client = None;
    let pw = self.secrets.get(AURA_KEY);
    self.obs.client = Some(AuraClient::start(self.obs.settings.clone(), pw));
    self.obs.view.status = "connecting";
    self.obs.view.detail.clear();
    self.emit_obs();
}
```

### [CTL-92] Déconnexion d'OBS

Arrête le client et remet la vue à zéro.

`app/src-tauri/src/controller/obs.rs` ligne 118

```rust
fn obs_disconnect(&mut self) {
    self.obs.client = None;
    self.obs.view = ObsView::idle("off", self.obs.settings.now_playing_source.clone());
    self.emit_obs();
}
```

### [CTL-93] Commande OBS

Envoie une commande si OBS est connecté, sinon prévient.

`app/src-tauri/src/controller/obs.rs` ligne 125

```rust
fn obs_send(&self, cmd: ObsCommand) {
    match &self.obs.client {
        Some(a) if self.obs.connected() => a.send(cmd),
        _ => self.notice(
            "warn",
            "OBS n'est pas connecté (panneau AURA OBS → Connecter).",
        ),
    }
}
```

### [CTL-94] Titre dans OBS

Écrit le titre en cours dans la source texte choisie.

`app/src-tauri/src/controller/obs.rs` ligne 136

```rust
pub(super) fn obs_now_playing(&self) {
    let src = self.obs.settings.now_playing_source.trim();
    if src.is_empty() || !self.obs.connected() || self.cast.song.is_empty() {
        return;
    }
    if let Some(a) = &self.obs.client {
        a.send(ObsCommand::SetText {
            source: src.to_string(),
            text: self.cast.song.clone(),
        });
    }
}
```

### [CTL-95] Actions OBS

Exécute les actions du panneau AURA · OBS.

`app/src-tauri/src/controller/obs.rs` ligne 150

```rust
pub(super) fn handle_obs(&mut self, action: ObsAction) {
    match action {
        ObsAction::Connect => self.obs_connect(),
        ObsAction::Disconnect => self.obs_disconnect(),
        ObsAction::Scene { name } => self.obs_send(ObsCommand::SetScene(name)),
        ObsAction::Stream => self.obs_send(ObsCommand::ToggleStream),
        ObsAction::Record => self.obs_send(ObsCommand::ToggleRecord),
        ObsAction::RecordPause => {
            if self.obs.view.record {
                self.obs_send(ObsCommand::ToggleRecordPause)
            } else {
                self.notice("warn", "Aucun enregistrement en cours dans OBS.")
…
```

### [CTL-96] Événements OBS

Applique les événements reçus et publie la vue.

`app/src-tauri/src/controller/obs.rs` ligne 177

```rust
pub(super) fn pump_obs(&mut self) {
    let Some(a) = &self.obs.client else {
        return;
    };
    let events = a.poll();
    if events.is_empty() {
        return;
    }
    let mut send_song = false;
    for e in events {
        send_song |= matches!(e, ObsEvent::Connected { .. });
        let level = if matches!(e, ObsEvent::Error(_)) {
…
```

### [CTL-97] Nouveaux réglages OBS

Reconnecte si les réglages changent ou après une erreur.

`app/src-tauri/src/controller/obs.rs` ligne 204

```rust
pub(super) fn apply_aura(&mut self, settings: AuraSettings) {
    let errored = self.obs.client.is_some() && self.obs.view.status == "error";
    if settings == self.obs.settings && !errored {
        return;
    }
    let reconnect = self.obs.client.is_some();
    self.obs.view.now_playing_source = settings.now_playing_source.clone();
    self.obs.settings = settings;
    if reconnect {
        self.obs_connect();
    } else {
        self.emit_obs();
…
```

### [CTL-98] Publication OBS

Envoie la vue OBS.

`app/src-tauri/src/controller/obs.rs` ligne 220

```rust
pub(super) fn emit_obs(&self) {
    self.emit(EV_OBS, &self.obs.view);
}
```

### [CTL-99] Pad chargé

Nom, couleur, fichier, son décodé ou erreur.

`app/src-tauri/src/controller/pads.rs` ligne 13

```rust
pub struct Pad {
    pub name: String,
    pub color: String,
    pub path: PathBuf,
    pub track: Option<Arc<Track>>,
    pub error: Option<String>,
}
```

### [CTL-100] État de la Sound FX

Pads et masque des pads en cours de lecture.

`app/src-tauri/src/controller/pads.rs` ligne 22

```rust
#[derive(Default)]
pub struct PadState {
    pub pads: HashMap<u16, Pad>,
    pub mask: u64,
}
```

### [CTL-101] Synchronisation des pads

Décode les pads ajoutés ou modifiés, arrête et retire les pads supprimés.

`app/src-tauri/src/controller/pads.rs` ligne 42

```rust
pub(super) fn sync_pads(&mut self, sb: &SoundboardSettings) {
    self.send(Command::SetSfxGain(db_to_gain(sb.volume_db)));
    let wanted: Vec<_> = sb
        .pads
        .iter()
        .filter(|p| p.index < PAD_COUNT && !p.path.as_os_str().is_empty())
        .collect();
    let gone: Vec<u16> = self
        .pads
        .pads
        .keys()
        .copied()
…
```

### [CTL-102] Pad décodé

Range le son décodé ou l'erreur.

`app/src-tauri/src/controller/pads.rs` ligne 99

```rust
pub(super) fn on_pad_decoded(
    &mut self,
    index: u16,
    path: PathBuf,
    result: Result<Track, String>,
) {
    let Some(p) = self.pads.pads.get_mut(&index).filter(|p| p.path == path) else {
        return;
    };
    match result {
        Ok(t) => {
            p.track = Some(Arc::new(t));
…
```

### [CTL-103] Pad pressé

Joue le son, l'arrête s'il joue déjà, ou explique pourquoi il n'est pas prêt.

`app/src-tauri/src/controller/pads.rs` ligne 119

```rust
pub(super) fn pad_play(&mut self, index: u16) {
    if self.pads.is_playing(index) {
        self.send(Command::StopSfx(index));
        return;
    }
    let Some(p) = self.pads.pads.get(&index) else {
        return;
    };
    match (&p.track, &p.error) {
        (Some(t), _) => self.send(Command::PlaySfx {
            pad: index,
            track: t.clone(),
…
```

### [CTL-104] Publication des pads

Envoie la vue de la Sound FX.

`app/src-tauri/src/controller/pads.rs` ligne 138

```rust
pub(super) fn emit_pads(&self) {
    let mut pads: Vec<PadView> = self
        .pads
        .pads
        .iter()
        .map(|(i, p)| PadView {
            index: *i,
            name: p.name.clone(),
            color: p.color.clone(),
            path: p.path.clone(),
            ready: p.track.is_some(),
            error: p.error.clone(),
…
```

### [CTL-105] Scan de fichiers

Développe dossiers et fichiers en entrées, en arrière-plan.

`app/src-tauri/src/controller/playlist_ops.rs` ligne 11

```rust
pub(super) fn scan(&self, paths: Vec<PathBuf>, index: Option<usize>, to_deck: Option<Deck>) {
    let tx = self.work_tx.clone();
    let cats = self.categories.clone();
    thread::spawn(move || {
        let files = playlist::expand(&paths);
        let entries = files
            .iter()
            .map(|p| playlist::make_entry(p, &cats))
            .collect();
        let _ = tx.send(Work::Scanned {
            entries,
            index,
…
```

### [CTL-106] Fichiers scannés

Enregistre en bibliothèque, charge le premier titre sur la platine visée, ajoute le reste à la playlist.

`app/src-tauri/src/controller/playlist_ops.rs` ligne 29

```rust
pub(super) fn on_scanned(
    &mut self,
    mut entries: Vec<Entry>,
    index: Option<usize>,
    to_deck: Option<Deck>,
) {
    if entries.is_empty() {
        self.notice("warn", "Aucun fichier audio trouvé.");
        return;
    }
    let errors = entries.iter().filter(|e| e.error.is_some()).count();
    if let Some(lib) = self.auto.history.as_mut() {
…
```

### [CTL-107] Publication de la playlist

Envoie la playlist avec le motif d'attente de chaque titre.

`app/src-tauri/src/controller/playlist_ops.rs` ligne 80

```rust

```

### [CTL-108] État du micro

Disponibilité, ouverture, niveau, ducking, nom et erreur.

`app/src-tauri/src/controller/voice.rs` ligne 8

```rust
pub struct MicState {
    pub available: bool,
    pub open: bool,
    pub peak: f32,
    pub duck: f32,
    pub name: Option<String>,
    pub error: Option<String>,
}
```

### [CTL-109] Micro demandé

Traduit les réglages en choix de micro pour le moteur.

`app/src-tauri/src/controller/voice.rs` ligne 32

```rust
pub fn mic_input(v: &VoiceSettings) -> MicInput {
    if !v.enabled {
        return MicInput::Off;
    }
    match v.input_device.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => MicInput::Named(name.to_string()),
        _ => MicInput::Default,
    }
}
```

### [CTL-110] Réglages de voix

Transmet gains, égaliseur, ducking, retour et préréglage au mixeur.

`app/src-tauri/src/controller/voice.rs` ligne 44

```rust
pub(super) fn apply_voice(&self, v: &VoiceSettings) {
    self.send(Command::SetMicParams(MicParams {
        gain_db: v.mic_gain_db,
        low_db: v.low_db,
        mid_db: v.mid_db,
        high_db: v.high_db,
        highpass: v.highpass,
    }));
    self.send(Command::SetMusicGain(db_to_gain(v.music_gain_db)));
    self.send(Command::SetDuck {
        enabled: v.duck,
        level_db: v.duck_db,
…
```

### [CTL-111] Ouverture du micro

Ouvre ou ferme le micro, ou prévient s'il n'y en a pas.

`app/src-tauri/src/controller/voice.rs` ligne 64

```rust
pub(super) fn mic_open(&mut self, open: bool) {
    if self.mic.available {
        self.mic.open = open;
        self.send(Command::SetMicOpen(open));
    } else if open {
        self.notice(
            "warn",
            "Aucun micro disponible : vérifie Réglages → Audio, puis redémarre BLAST.",
        );
    }
}
```

### [CTL-121] Enregistrement d'un passage

Point d'entrée unique des passages : base et tampons mémoire en même temps ; si la base échoue, le passage reste en mémoire et l'erreur va au journal.

`app/src-tauri/src/controller/automation.rs` ligne 194

```rust
pub(super) fn record_play(&mut self, play: NewPlay) -> Option<i64> {
    let history = self.auto.history.as_mut()?;
    let (id, error) = history.record(play);
    if let Some(e) = error {
        journal::warn(format!(
            "historique (passage gardé en mémoire seulement) : {e}"
        ));
    }
    Some(id)
}
```

### [CTL-122] Fin d'un passage

Enregistre la durée réellement diffusée, en base et en mémoire.

`app/src-tauri/src/controller/automation.rs` ligne 206

```rust
pub(super) fn finish_play(&mut self, id: i64, duration: f64) {
    if let Some(history) = self.auto.history.as_mut() {
        if let Err(e) = history.finish(id, duration) {
            journal::warn(format!("historique (fin de passage) : {e}"));
        }
    }
}
```

### [CTL-123] Vue d'un jingle

Nom, fichier, durée, prêt, en erreur ou en cours de lecture.

`app/src-tauri/src/controller/views.rs` ligne 196

```rust
#[derive(Serialize, Clone)]
pub struct JingleView {
    pub index: usize,
    pub name: String,
    pub path: PathBuf,
    pub duration: Option<f64>,
    pub ready: bool,
    pub error: Option<String>,
    pub playing: bool,
}
```

### [CTL-124] Vue de la box à jingles

Jingles du dossier, dossier choisi et lecture en cours du dossier.

`app/src-tauri/src/controller/views.rs` ligne 208

```rust
#[derive(Serialize, Clone)]
pub struct JinglesView {
    pub items: Vec<JingleView>,
    pub dirs: Vec<PathBuf>,
    pub scanning: bool,
}
```

### [CTL-125] Nombre maximal de jingles

Au-delà de 500 fichiers, le reste du dossier est ignoré.

`app/src-tauri/src/controller/jingles.rs` ligne 16

```rust
const MAX_JINGLES: usize = 500;
```

### [CTL-126] Jingle chargé

Entrée du dossier, son décodé ou erreur.

`app/src-tauri/src/controller/jingles.rs` ligne 19

```rust
pub struct Jingle {
    pub entry: Entry,
    pub track: Option<Arc<Track>>,
    pub error: Option<String>,
}
```

### [CTL-127] Jingle en cours

Ligne d'historique et heure de départ d'un jingle lancé depuis la box.

`app/src-tauri/src/controller/jingles.rs` ligne 26

```rust
pub struct Cart {
    pub hist_id: Option<i64>,
    pub started: Instant,
}
```

### [CTL-128] État de la box à jingles

Dossier, jingles, génération du dernier scan et jingles en cours.

`app/src-tauri/src/controller/jingles.rs` ligne 32

```rust
#[derive(Default)]
pub struct JingleState {
    pub dirs: Vec<PathBuf>,
    pub items: Vec<Jingle>,
    pub generation: u64,
    pub scanning: bool,
    pub playing: HashMap<usize, Cart>,
}
```

### [CTL-129] Synchronisation des jingles

Applique le volume des jingles et relit le dossier s'il a changé.

`app/src-tauri/src/controller/jingles.rs` ligne 47

```rust
pub(super) fn sync_jingles(&mut self, settings: &JingleSettings) {
    self.send(Command::SetCartGain(db_to_gain(settings.volume_db)));
    if settings.dirs != self.jingles.dirs {
        self.jingles.dirs = settings.dirs.clone();
        self.rescan_jingles();
    }
}
```

### [CTL-130] Lecture du dossier de jingles

Arrête les jingles en cours, liste le dossier puis décode chaque jingle en arrière-plan ; un scan plus récent annule le précédent.

`app/src-tauri/src/controller/jingles.rs` ligne 56

```rust
pub(super) fn rescan_jingles(&mut self) {
    self.jingle_stop_all();
    self.jingles.generation += 1;
    self.jingles.items.clear();
    self.jingles.scanning = !self.jingles.dirs.is_empty();
    self.emit_jingles();
    if self.jingles.dirs.is_empty() {
        return;
    }
    let generation = self.jingles.generation;
    let dirs = self.jingles.dirs.clone();
    let cats = CategorySettings {
…
```

### [CTL-131] Lancement d'un jingle

Joue le jingle à l'antenne par-dessus la musique, avec son propre volume, et l'inscrit dans l'historique ; un second clic l'arrête.

`app/src-tauri/src/controller/jingles.rs` ligne 144

```rust
pub(super) fn jingle_play(&mut self, index: usize) {
    if self.jingles.playing.contains_key(&index) {
        self.send(Command::StopSfx(voice(index)));
        self.on_cart_ended(index);
        return;
    }
    let Some(j) = self.jingles.items.get(index) else {
        return;
    };
    let track = match (&j.track, &j.error) {
        (Some(t), _) => t.clone(),
        (None, Some(e)) => {
…
```

### [CTL-132] Arrêt des jingles

Coupe tous les jingles en cours et clôt leurs passages.

`app/src-tauri/src/controller/jingles.rs` ligne 192

```rust
pub(super) fn jingle_stop_all(&mut self) {
    self.send(Command::StopAllCarts);
    let playing: Vec<usize> = self.jingles.playing.keys().copied().collect();
    for index in playing {
        self.on_cart_ended(index);
    }
}
```

### [CTL-133] Fin d'un jingle

Clôt le passage du jingle avec sa durée réelle ; les fins de sons de pads restent hors historique.

`app/src-tauri/src/controller/jingles.rs` ligne 201

```rust
pub(super) fn on_cart_ended(&mut self, index: usize) {
    let Some(cart) = self.jingles.playing.remove(&index) else {
        return;
    };
    if let Some(id) = cart.hist_id {
        self.finish_play(id, cart.started.elapsed().as_secs_f64());
    }
    self.emit_jingles();
    self.emit_history();
}
```

### [CTL-134] Publication de la box à jingles

Envoie la vue de la box à jingles.

`app/src-tauri/src/controller/jingles.rs` ligne 221

```rust
pub(super) fn emit_jingles(&self) {
    let items = self
        .jingles
        .items
        .iter()
        .enumerate()
        .map(|(index, j)| JingleView {
            index,
            name: j.entry.title.clone(),
            path: j.entry.path.clone(),
            duration: j.entry.duration,
            ready: j.track.is_some(),
…
```

### [CTL-135] Avance de la playlist

Avec une grille active, l'auto-mix garde au moins 3 titres d'avance dans la playlist.

`app/src-tauri/src/controller/rotation.rs` ligne 17

```rust
const LOOKAHEAD: usize = 3;
```

### [CTL-136] Lot de scan

Les titres lus sont enregistrés en base par lots de 200.

`app/src-tauri/src/controller/rotation.rs` ligne 19

```rust
const SCAN_BATCH: usize = 200;
```

### [CTL-137] Tentatives de tirage

Nombre de tirages tentés si le fichier choisi a disparu du disque.

`app/src-tauri/src/controller/rotation.rs` ligne 21

```rust
const PICK_ATTEMPTS: usize = 8;
```

### [CTL-138] État des grilles

Grilles, dossiers et titres de la bibliothèque en mémoire, scan en cours, grille et case courantes, tirage aléatoire et avertissements déjà donnés.

`app/src-tauri/src/controller/rotation.rs` ligne 24

```rust
pub struct RotationState {
    pub rotation: Rotation,
    pub library_dirs: Vec<PathBuf>,
    pub tracks: Vec<LibraryTrack>,
    pub index: HashMap<String, usize>,
    pub scanning: bool,
    pub scanned: usize,
    pub clock_id: Option<String>,
    pub cursor: usize,
    pub fb_cursor: usize,
    pub rng: u64,
    pub warned: HashSet<String>,
…
```

### [CTL-139] Passage d'un titre

Met à jour en mémoire la date du dernier passage, pour que la pondération en tienne compte aussitôt.

`app/src-tauri/src/controller/rotation.rs` ligne 108

```rust
pub fn mark_played(&mut self, path: &str, at: i64) {
    if let Some(&i) = self.index.get(path) {
        self.tracks[i].last_played = Some(at);
        self.tracks[i].play_count += 1;
    }
}
```

### [CTL-140] Scan de la bibliothèque

Parcourt les dossiers en arrière-plan et ne relit que les fichiers nouveaux ou modifiés (date de modification).

`app/src-tauri/src/controller/rotation.rs` ligne 117

```rust
fn scan_library(
    dirs: Vec<PathBuf>,
    cats: CategorySettings,
    known: HashMap<String, i64>,
    tx: crossbeam_channel::Sender<Work>,
) {
    let files = playlist::expand(&dirs);
    let mut batch: Vec<TrackMeta> = Vec::with_capacity(SCAN_BATCH);
    for f in &files {
        let mtime = playlist::file_mtime(f);
        if mtime.is_some() && known.get(&*f.to_string_lossy()) == mtime.as_ref() {
            continue;
…
```

### [CTL-141] Chargement de la bibliothèque

Charge en mémoire les titres connus de la base.

`app/src-tauri/src/controller/rotation.rs` ligne 148

```rust
pub(super) fn load_library_tracks(&mut self) {
    let Some(history) = self.auto.history.as_ref() else {
        return;
    };
    match history.tracks() {
        Ok(t) => self.rot.set_tracks(t),
        Err(e) => journal::warn(format!("bibliothèque (lecture) : {e}")),
    }
}
```

### [CTL-142] Relecture de la bibliothèque

Lance un scan des dossiers de la bibliothèque et des playlists M3U des catégories et émissions, sauf s'il en tourne déjà un.

`app/src-tauri/src/controller/rotation.rs` ligne 159

```rust
pub(super) fn library_rescan(&mut self) {
    if self.rot.scanning {
        self.notice("info", "Scan de la bibliothèque déjà en cours…");
        return;
    }
    let targets = self.rot.scan_targets();
    if targets.is_empty() {
        self.emit_library();
        return;
    }
    let known = self
        .auto
…
```

### [CTL-143] Nouveaux réglages de grilles

Applique les grilles (la case courante repart du début) et relit la bibliothèque si ses dossiers changent.

`app/src-tauri/src/controller/rotation.rs` ligne 205

```rust
pub(super) fn apply_rotation(&mut self, rotation: Rotation, library: &LibrarySettings) {
    let mut rescan = false;
    if rotation != self.rot.rotation {
        rescan = rotation.playlist_paths() != self.rot.rotation.playlist_paths();
        self.rot.rotation = rotation;
        self.rot.clock_id = None;
        self.rot.warned.clear();
        self.rot.refresh_lists();
        self.emit_state();
    }
    if library.dirs != self.rot.library_dirs {
        self.rot.library_dirs = library.dirs.clone();
…
```

### [CTL-144] Grille de l'heure

Grille du planning pour le jour et l'heure locaux, ou grille hors planning.

`app/src-tauri/src/controller/rotation.rs` ligne 225

```rust
pub(super) fn current_clock(&self) -> Option<ClockView> {
    let now = chrono::Local::now();
    let wd = now.weekday().num_days_from_monday() as usize;
    scheduled(&self.rot.rotation, wd, now.hour() as usize).map(|s| match s {
        Scheduled::Clock(c) => ClockView {
            id: c.id.clone(),
            name: c.name.clone(),
            color: c.color.clone(),
            program: false,
        },
        Scheduled::Program(p) => ClockView {
            id: p.id.clone(),
…
```

### [CTL-145] Remplissage par la grille

Ajoute à la playlist les titres de la grille ou de l'émission de l'heure jusqu'à avoir 3 titres d'avance ; tout repart du début quand l'heure change de contenu.

`app/src-tauri/src/controller/rotation.rs` ligne 245

```rust
pub(super) fn rotation_fill(&mut self) -> bool {
    let Some(clock) = self.current_clock() else {
        self.rot.clock_id = None;
        return false;
    };
    if self.rot.clock_id.as_deref() != Some(clock.id.as_str()) {
        self.rot.clock_id = Some(clock.id.clone());
        self.rot.cursor = 0;
        self.rot.fb_cursor = 0;
        self.rot.program_done = false;
        self.rot.program = Vec::new();
        if clock.program {
…
```

### [CTL-146] Tirage dans une catégorie

Titres de la catégorie absents de la playlist et des platines, conformes aux règles de séparation (et pas du même artiste qu'un titre déjà prévu), tirés avec pondération ; si aucun n'est conforme, les règles sont assouplies avec un avertissement.

`app/src-tauri/src/controller/rotation.rs` ligne 381

```rust
fn pick_from(&mut self, cat: &RotationCategory, pending: &[Entry]) -> Option<Entry> {
    let busy: HashSet<PathBuf> = self
        .playlist
        .entries()
        .iter()
        .chain(pending)
        .map(|e| e.path.clone())
        .chain(self.decks.iter().filter_map(|d| d.meta.path.clone()))
        .collect();
    let queued_artists: HashSet<String> = self
        .playlist
        .entries()
…
```

### [CTL-147] Publication de la bibliothèque

Envoie dossiers, nombre de titres et état du scan.

`app/src-tauri/src/controller/rotation.rs` ligne 456

```rust
pub(super) fn emit_library(&self) {
    self.emit(
        EV_LIBRARY,
        &LibraryView {
            dirs: self.rot.library_dirs.clone(),
            tracks: self.rot.tracks.len(),
            scanning: self.rot.scanning,
            scanned: self.rot.scanned,
        },
    );
}
```

### [CTL-148] Vue de la grille active

Identifiant, nom, couleur de la grille ou de l'émission de l'heure, et s'il s'agit d'une émission.

`app/src-tauri/src/controller/views.rs` ligne 115

```rust
#[derive(Serialize, Clone)]
pub struct ClockView {
    pub id: String,
    pub name: String,
    pub color: String,
    pub program: bool,
}
```

### [CTL-149] Vue de la bibliothèque

Dossiers, nombre de titres, scan en cours et titres lus.

`app/src-tauri/src/controller/views.rs` ligne 124

```rust
#[derive(Serialize, Clone)]
pub struct LibraryView {
    pub dirs: Vec<PathBuf>,
    pub tracks: usize,
    pub scanning: bool,
    pub scanned: usize,
}
```

### [CTL-150] Règles pour un titre

Vérifie un titre décrit par son fichier, son titre, son artiste et son type : platines puis historique ; commun à la playlist et aux grilles.

`app/src-tauri/src/controller/automation.rs` ligne 87

```rust
pub(super) fn violation_of(
    &self,
    path: &str,
    title: &str,
    artist: &str,
    kind: Kind,
    now: i64,
) -> Option<String> {
    if kind != Kind::Morceau {
        return None;
    }
    let rules = self.auto.rules();
…
```

### [CTL-151] Mélange de la playlist

Mélange simple ou en espaçant les artistes, avec le tirage aléatoire du contrôleur.

`app/src-tauri/src/controller/playlist_ops.rs` ligne 81

```rust
pub(super) fn shuffle(&mut self, spread: bool) {
    let rot = &mut self.rot;
    let mut roll = || rot.roll();
    if spread {
        self.playlist.shuffle_spread(&mut roll);
    } else {
        self.playlist.shuffle(&mut roll);
    }
    self.emit_playlist();
}
```

### [CTL-152] Doublons de la playlist

Retire les doublons et indique combien ont été retirés.

`app/src-tauri/src/controller/playlist_ops.rs` ligne 93

```rust
pub(super) fn dedupe(&mut self) {
    let n = self.playlist.dedupe();
    let msg = match n {
        0 => "Aucun doublon dans la playlist.".to_string(),
        1 => "1 doublon retiré de la playlist.".to_string(),
        n => format!("{n} doublons retirés de la playlist."),
    };
    self.notice("info", msg);
    self.emit_playlist();
}
```

### [CTL-153] Vue des motifs d'attente

Liste compacte (id, motif) des entrées bloquées par les règles de rotation.

`app/src-tauri/src/controller/views.rs` ligne 109

```rust
#[derive(Serialize, Clone)]
pub struct BlockedView {
    pub blocked: Vec<(u64, String)>,
}
```

### [CTL-154] Publication des motifs d'attente

Émet blast://blocked sans renvoyer toute la playlist.

`app/src-tauri/src/controller/playlist_ops.rs` ligne 123

```rust
pub(super) fn emit_blocked(&self) {
    let now = now_secs();
    let blocked = self
        .playlist
        .entries()
        .iter()
        .filter_map(|e| self.violation_for(e, now).map(|why| (e.id, why)))
        .collect();
    self.emit(EV_BLOCKED, &BlockedView { blocked });
}
```

### [CTL-155] Délai de sauvegarde

Temps maximal entre une modification de la playlist et son écriture sur disque.

`app/src-tauri/src/controller/queue.rs` ligne 12

```rust
const SAVE_AFTER: Duration = Duration::from_secs(2);
```

### [CTL-156] Instantané de la playlist

Copie des entrées et des titres des platines envoyée au fil de sauvegarde.

`app/src-tauri/src/controller/queue.rs` ligne 15

```rust
struct Snapshot {
    entries: Vec<Entry>,
    decks: Vec<PathBuf>,
}
```

### [CTL-157] État de la sauvegarde

Chemin de playlist.json, canal du fil de sauvegarde et heure de la première modification non écrite.

`app/src-tauri/src/controller/queue.rs` ligne 21

```rust
#[derive(Default)]
pub struct QueueState {
    path: Option<PathBuf>,
    tx: Option<Sender<Snapshot>>,
    dirty: Cell<Option<Instant>>,
}
```

### [CTL-158] Fil de sauvegarde

Écrit le dernier instantané reçu, hors du fil du contrôleur.

`app/src-tauri/src/controller/queue.rs` ligne 37

```rust
fn saver(path: PathBuf, rx: Receiver<Snapshot>) {
    while let Ok(mut snap) = rx.recv() {
        while let Ok(newer) = rx.try_recv() {
            snap = newer;
        }
        if let Err(e) = save_queue(&path, &snap.entries, &snap.decks) {
            journal::warn(format!(
                "sauvegarde de la playlist {} : {e}",
                path.display()
            ));
        }
    }
…
```

### [CTL-159] Reprise de la playlist

Au démarrage, recharge playlist.json : titres des platines en tête, puis la playlist, reclassée selon les catégories.

`app/src-tauri/src/controller/queue.rs` ligne 53

```rust
pub(super) fn restore_queue(&mut self, path: PathBuf) {
    if path.exists() {
        match load_queue(&path) {
            Ok((entries, decks)) => {
                let mut items: Vec<Entry> = decks
                    .iter()
                    .filter(|p| p.exists())
                    .map(|p| make_entry(p, &self.categories))
                    .collect();
                items.extend(entries);
                let n = items.len();
                self.playlist.insert(items, None);
…
```

### [CTL-160] Prise d'instantané

Copie la playlist et les titres en cours sur les platines.

`app/src-tauri/src/controller/queue.rs` ligne 86

```rust
fn snapshot(&self) -> Snapshot {
    Snapshot {
        entries: self.playlist.entries().to_vec(),
        decks: self
            .decks
            .iter()
            .filter(|d| !d.meta.finished)
            .filter_map(|d| d.meta.path.clone())
            .collect(),
    }
}
```

### [CTL-161] Sauvegarde différée

Envoie un instantané au fil de sauvegarde quand le délai est écoulé.

`app/src-tauri/src/controller/queue.rs` ligne 99

```rust
pub(super) fn save_queue_if_due(&mut self) {
    let due = self
        .queue
        .dirty
        .get()
        .is_some_and(|t| t.elapsed() >= SAVE_AFTER);
    if !due {
        return;
    }
    self.queue.dirty.set(None);
    let snap = self.snapshot();
    if let Some(tx) = &self.queue.tx {
…
```

### [CTL-162] Sauvegarde immédiate

Écrit la playlist tout de suite, à la fermeture de l'application.

`app/src-tauri/src/controller/queue.rs` ligne 116

```rust
pub(super) fn flush_queue(&mut self) {
    if self.queue.dirty.take().is_none() {
        return;
    }
    let Some(path) = self.queue.path.clone() else {
        return;
    };
    let snap = self.snapshot();
    if let Err(e) = save_queue(&path, &snap.entries, &snap.decks) {
        journal::warn(format!(
            "sauvegarde de la playlist {} : {e}",
            path.display()
…
```

### [CTL-163] Enregistrement en M3U

Écrit la playlist dans le fichier M3U choisi et indique le nombre de titres.

`app/src-tauri/src/controller/playlist_ops.rs` ligne 105

```rust
pub(super) fn save_m3u(&mut self, path: PathBuf) {
    let entries = self.playlist.entries();
    match playlist::write_m3u(&path, entries) {
        Ok(()) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.notice(
                "info",
                format!("Playlist enregistrée : {name} ({} titres).", entries.len()),
            );
…
```

### [CTL-164] Lecture des playlists de catégories

Relit les M3U des catégories pour savoir quels titres de la bibliothèque y figurent.

`app/src-tauri/src/controller/rotation.rs` ligne 78

```rust
pub fn refresh_lists(&mut self) {
    self.lists = self
        .rotation
        .categories
        .iter()
        .flat_map(|c| c.rule.playlists.iter())
        .map(|p| {
            let set = playlist::read_m3u(Path::new(p))
                .iter()
                .map(|t| path_key(&t.to_string_lossy()))
                .collect();
            (path_key(p), set)
…
```

### [CTL-165] Début d'émission

Lit le M3U de l'émission quand son heure commence ; prévient s'il est vide ou introuvable.

`app/src-tauri/src/controller/rotation.rs` ligne 288

```rust
fn start_program(&mut self, id: &str) {
    let Some(p) = self.rot.rotation.program(id).cloned() else {
        return;
    };
    self.rot.program = playlist::read_m3u(Path::new(&p.path));
    if self.rot.program.is_empty() {
        self.notice(
            "warn",
            format!(
                "Émission « {} » : playlist {} vide ou introuvable.",
                p.name, p.path
            ),
…
```

### [CTL-166] Titre suivant de l'émission

Prend le titre suivant du M3U dans l'ordre, en sautant les fichiers absents ; tags de la bibliothèque s'il y est connu.

`app/src-tauri/src/controller/rotation.rs` ligne 311

```rust
fn program_next(&mut self, tag: SlotTag) -> Option<Entry> {
    while self.rot.cursor < self.rot.program.len() {
        let path = self.rot.program[self.rot.cursor].clone();
        self.rot.cursor += 1;
        if !path.is_file() {
            continue;
        }
        let known = self.rot.index.get(&*path.to_string_lossy()).copied();
        let entry = match known {
            Some(i) => playlist::from_library(&self.rot.tracks[i], tag),
            None => Entry {
                slot: Some(tag),
…
```

### [CTL-167] Fin d'émission

Quand le M3U est épuisé, annonce la fin une fois et continue avec la grille hors planning.

`app/src-tauri/src/controller/rotation.rs` ligne 332

```rust
fn fallback_next(&mut self, program: &str, pending: &[Entry]) -> Option<Entry> {
    if !self.rot.program_done {
        self.rot.program_done = true;
        if !self.rot.program.is_empty() {
            self.notice(
                "info",
                format!("Émission « {program} » terminée : retour à la grille par défaut."),
            );
        }
    }
    let id = default_clock(&self.rot.rotation)?.id.clone();
    self.rotation_next(&id, pending, true)
…
```

### [CTL-168] Limites du chat

Messages gardés en mémoire, délais entre deux !np et deux !sfx, étiquette et couleur des demandes acceptées.

`app/src-tauri/src/controller/chat.rs` ligne 15

```rust
const KEEP: usize = 300;
```

### [CTL-169] Demande de titre

Viewer, recherche tapée, titre trouvé dans la bibliothèque (ou rien) et heure.

`app/src-tauri/src/controller/chat.rs` ligne 22

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SongRequest {
    pub id: u64,
    pub user: String,
    pub login: String,
    pub query: String,
    pub path: Option<PathBuf>,
    pub artist: String,
    pub title: String,
    pub at_ms: u64,
}
```

### [CTL-170] Vue du chat

Chaîne, connexion, écriture possible, connexion demandée, détail et demandes en attente.

`app/src-tauri/src/controller/chat.rs` ligne 35

```rust
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatView {
    pub channel: String,
    pub connected: bool,
    pub writable: bool,
    pub active: bool,
    pub detail: String,
    pub requests: Vec<SongRequest>,
}
```

### [CTL-171] État du chat

Réglages, client Twitch, vue, derniers messages, numéro de connexion pour ignorer les événements d'une connexion fermée, délais des commandes.

`app/src-tauri/src/controller/chat.rs` ligne 46

```rust
pub struct ChatState {
    pub settings: ChatSettings,
    pub client: Option<TwitchClient>,
    pub view: ChatView,
    pub history: VecDeque<ChatEvent>,
    next_request: u64,
    generation: u64,
    last_np: Option<Instant>,
    last_sfx: Option<Instant>,
    #[cfg(test)]
    pub sent: Vec<String>,
}
```

### [CTL-172] Texte comparable

Minuscules sans accents ni ponctuation, pour comparer une demande aux titres de la bibliothèque.

`app/src-tauri/src/controller/chat.rs` ligne 80

```rust
fn fold(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ÿ' => 'y',
            c if c.is_alphanumeric() => c,
…
```

### [CTL-173] Recherche d'un titre

Chaque mot de la demande doit commencer un mot de « artiste titre » ; un titre ou un artiste exact donne des points ; seuls les morceaux comptent.

`app/src-tauri/src/controller/chat.rs` ligne 101

```rust
pub fn find_track(tracks: &[LibraryTrack], query: &str) -> Option<usize> {
    let wanted = fold(&query.to_lowercase());
    let words: Vec<&str> = wanted.split(' ').filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return None;
    }
    let (artist_part, title_part) = match query.split_once(" - ") {
        Some((a, t)) => (fold(&a.to_lowercase()), fold(&t.to_lowercase())),
        None => (String::new(), wanted.clone()),
    };
    tracks
        .iter()
…
```

### [CTL-174] Application des réglages du chat

Garde les nouveaux réglages et reconnecte si la chaîne ou le compte a changé.

`app/src-tauri/src/controller/chat.rs` ligne 146

```rust
pub(super) fn apply_chat(&mut self, settings: ChatSettings) {
    if settings == self.chat.settings {
        return;
    }
    let reconnect = self.chat.client.is_some()
        && (settings.channel != self.chat.settings.channel
            || settings.bot_login != self.chat.settings.bot_login);
    self.chat.settings = settings;
    if !self.chat.view.active {
        self.chat.view.channel =
            blast_live::twitch::normalize_channel(&self.chat.settings.channel);
    }
…
```

### [CTL-175] Connexion au chat

Ferme la connexion précédente et lance le client Twitch, avec le jeton du trousseau si un compte est indiqué.

`app/src-tauri/src/controller/chat.rs` ligne 166

```rust
pub(super) fn chat_connect(&mut self) {
    self.chat_disconnect_quiet();
    let token = if self.chat.settings.bot_login.trim().is_empty() {
        String::new()
    } else {
        self.secrets.get(CHAT_KEY).unwrap_or_default()
    };
    let cfg = TwitchConfig::new(
        &self.chat.settings.channel,
        &self.chat.settings.bot_login,
        &token,
    );
…
```

### [CTL-176] Déconnexion du chat

Arrête le client et publie l'état déconnecté.

`app/src-tauri/src/controller/chat.rs` ligne 204

```rust
pub(super) fn chat_disconnect(&mut self) {
    self.chat_disconnect_quiet();
    self.chat.view.active = false;
    self.chat.view.connected = false;
    self.chat.view.detail = "déconnecté".into();
    self.emit_chat_state();
}
```

### [CTL-177] Envoi dans le chat

Envoie un message par le client Twitch.

`app/src-tauri/src/controller/chat.rs` ligne 213

```rust
pub(super) fn chat_say(&mut self, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    #[cfg(test)]
    self.chat.sent.push(text.to_string());
    if let Some(c) = &self.chat.client {
        c.say(text);
    }
}
```

### [CTL-178] Événement de chat reçu

Met à jour l'état, l'historique (effacements compris), relaie l'événement à l'interface et traite les commandes des viewers.

`app/src-tauri/src/controller/chat.rs` ligne 230

```rust
pub(super) fn on_chat(&mut self, generation: u64, e: ChatEvent) {
    if generation != self.chat.generation {
        return;
    }
    match &e {
        ChatEvent::Status {
            connected,
            writable,
            detail,
        } => {
            self.chat.view.connected = *connected;
            self.chat.view.writable = *writable;
…
```

### [CTL-179] Commandes des viewers

!np répond avec le titre à l'antenne (une fois toutes les 15 s), !request crée une demande, !sfx lance le pad du même nom (une fois toutes les 10 s), chacune selon les réglages.

`app/src-tauri/src/controller/chat.rs` ligne 273

```rust
fn chat_command(&mut self, m: &ChatMessage) {
    let Some(cmd) = parse_command(&m.text) else {
        return;
    };
    match cmd {
        ChatCommand::NowPlaying if self.chat.settings.cmd_np => {
            if self.chat.last_np.is_some_and(|t| t.elapsed() < NP_COOLDOWN) || !self.can_reply()
            {
                return;
            }
            self.chat.last_np = Some(Instant::now());
            let song = self.now_playing();
…
```

### [CTL-180] Nouvelle demande

Applique la limite de demandes par viewer, cherche le titre, ajoute la demande à la file et répond au viewer.

`app/src-tauri/src/controller/chat.rs` ligne 326

```rust
fn chat_request(&mut self, m: &ChatMessage, query: &str) {
    let pending = self
        .chat
        .view
        .requests
        .iter()
        .filter(|r| r.login == m.login && r.path.is_some())
        .count() as u32;
    let limit = self.chat.settings.request_limit.max(1);
    if pending >= limit {
        if self.can_reply() {
            self.chat_say(&format!(
…
```

### [CTL-181] Demande acceptée

Ajoute le titre en tête de playlist, étiqueté « Demande » et exempté des règles de séparation, puis prévient le viewer.

`app/src-tauri/src/controller/chat.rs` ligne 376

```rust
pub(super) fn request_accept(&mut self, id: u64) {
    let Some(pos) = self.chat.view.requests.iter().position(|r| r.id == id) else {
        return;
    };
    let r = self.chat.view.requests.remove(pos);
    let Some(path) = r.path.clone() else {
        self.emit_chat_state();
        return;
    };
    if !path.is_file() {
        self.notice(
            "warn",
…
```

### [CTL-182] Demande refusée

Retire la demande de la file.

`app/src-tauri/src/controller/chat.rs` ligne 419

```rust
pub(super) fn request_reject(&mut self, id: u64) {
    self.chat.view.requests.retain(|r| r.id != id);
    self.emit_chat_state();
}
```

### [CTL-183] Annonce du titre

Écrit « En cours : … » dans le chat à chaque nouveau morceau si l'option est active.

`app/src-tauri/src/controller/chat.rs` ligne 425

```rust
pub(super) fn chat_announce(&mut self, song: &str) {
    if self.chat.settings.announce && self.chat.client.is_some() && self.can_reply() {
        self.chat_say(&format!("🎵 En cours : {song}"));
    }
}
```

### [CTL-184] Publication du chat

Envoie à l'interface l'état du chat et, au démarrage de l'interface, les derniers messages.

`app/src-tauri/src/controller/chat.rs` ligne 432

```rust
pub(super) fn emit_chat_state(&self) {
    self.ui.emit(
        EV_CHAT_STATE,
        serde_json::to_value(&self.chat.view).unwrap_or_default(),
    );
}
```


## UI

Interface : modules JavaScript

### [UI-01] Accès à Tauri

Objets Tauri, raccourcis $ et $$, icônes du sprite.

`app/ui/js/core.js` ligne 1

```js
export const T = window.__TAURI__;
```

### [UI-02] État partagé

Dernier état reçu, entrées de playlist et réglages ouverts.

`app/ui/js/core.js` ligne 12

```js
export const S = {
  state: null,
  entries: [],
  settings: null,
};
```

### [UI-03] Envoi d'une commande

Envoie une commande au contrôleur ; une erreur s'affiche en notification.

`app/ui/js/core.js` ligne 19

```js
export function send(cmd) {
  return invoke("blast", { cmd }).catch((e) => toast("error", String(e)));
}
```

### [UI-04] Notification

Affiche un message temporaire, avec boutons d'action éventuels.

`app/ui/js/core.js` ligne 31

```js
export function toast(level, message, actions = []) {
  const el = document.createElement("div");
  el.className = `toast ${level}`;
  const text = document.createElement("span");
  text.textContent = message;
  el.appendChild(text);
  for (const a of actions) {
    const b = document.createElement("button");
    b.className = "btn ghost";
    b.textContent = a.label;
    b.addEventListener("click", () => {
      a.run();
…
```

### [UI-05] Durée

Formate des secondes en mm:ss ou h:mm:ss.

`app/ui/js/core.js` ligne 54

```js
export function fmt(secs) {
  if (!isFinite(secs) || secs < 0) secs = 0;
  const s = Math.floor(secs);
  const h = Math.floor(s / 3600);
  const mm = String(Math.floor((s % 3600) / 60)).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}
```

### [UI-06] Chronomètre

Formate des millisecondes en hh:mm:ss.

`app/ui/js/core.js` ligne 64

```js
export function hms(ms) {
  const s = Math.max(0, Math.floor(ms / 1000));
  return [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60].map((n) => String(n).padStart(2, "0")).join(":");
}
```

### [UI-07] Échelle des vumètres

Convertit une crête en largeur de vumètre sur 48 dB.

`app/ui/js/core.js` ligne 70

```js
export function dbWidth(peak) {
  if (peak <= 0.00001) return 0;
  const db = 20 * Math.log10(peak);
  return Math.max(0, Math.min(1, (db + 48) / 48));
}
```

### [UI-08] Préférences locales

Lecture et écriture protégées dans localStorage.

`app/ui/js/core.js` ligne 77

```js
export const store = {
  get(k) {
    try { return localStorage.getItem(k); } catch (_) { return null; }
  },
  set(k, v) {
    try { localStorage.setItem(k, v); } catch (_) {}
  },
};
```

### [UI-09] Lecture de formulaire

Valeurs d'un formulaire selon le type des champs.

`app/ui/js/core.js` ligne 87

```js
export function formValues(form) {
  const v = {};
  for (const el of form.elements) {
    if (!el.name) continue;
    if (el.type === "checkbox") v[el.name] = el.checked;
    else if (el.type === "number") v[el.name] = Number(el.value);
    else v[el.name] = el.value;
  }
  return v;
}
```

### [UI-10] Remplissage de formulaire

Remplit un formulaire à partir d'un objet.

`app/ui/js/core.js` ligne 99

```js
export function fillForm(form, data) {
  for (const el of form.elements) {
    if (!el.name || !(el.name in data)) continue;
    if (el.type === "checkbox") el.checked = !!data[el.name];
    else el.value = String(data[el.name]);
  }
}
```

### [UI-11] Résultat de formulaire

Affiche un message de réussite, d'attente ou d'erreur.

`app/ui/js/core.js` ligne 108

```js
export function setResult(id, kind, text) {
  const el = $(id);
  el.className = `form-result ${kind}`;
  el.textContent = text;
}
```

### [UI-12] Liste déroulante

Remplit une liste à partir de paires identifiant / libellé.

`app/ui/js/core.js` ligne 115

```js
export function fillSelect(sel, items, value) {
  sel.innerHTML = "";
  for (const [id, label] of items) {
    const o = document.createElement("option");
    o.value = id;
    o.textContent = label;
    sel.appendChild(o);
  }
  sel.value = value;
}
```

### [UI-13] Double confirmation

Demande un second clic dans les 4 secondes avant une action destructive.

`app/ui/js/core.js` ligne 127

```js
export function confirmClick(btn, label, confirmLabel, run) {
  if (btn.dataset.confirm !== "1") {
    btn.dataset.confirm = "1";
    btn.textContent = confirmLabel;
    setTimeout(() => {
      btn.dataset.confirm = "";
      btn.textContent = label;
    }, 4000);
    return;
  }
  btn.dataset.confirm = "";
  btn.textContent = label;
…
```

### [UI-14] Choix de fichiers

Ouvre le sélecteur de fichiers audio du système.

`app/ui/js/core.js` ligne 143

```js
export async function pickAudio(multiple, title) {
  const picked = await T.dialog.open({ multiple, directory: false, title, filters: [{ name: "Audio", extensions: AUDIO_EXT }] });
  return picked == null ? [] : [].concat(picked);
}
```

### [UI-15] Horloge

Met à jour l'horloge de la barre du haut.

`app/ui/js/main.js` ligne 19

```js
function tickClock() {
  $("#clock").textContent = new Date().toLocaleTimeString("fr-FR", { hour12: false });
}
```

### [UI-16] Abonnements

Relie chaque événement blast:// à son module.

`app/ui/js/main.js` ligne 24

```js
async function subscribe() {
  await listen("blast://state", (e) => {
    S.state = e.payload;
    decks.renderState();
  });
  await listen("blast://playlist", (e) => {
    S.entries = e.payload.entries;
    playlist.renderPlaylist(e.payload.total);
  });
  await listen("blast://blocked", (e) => playlist.setBlocked(e.payload.blocked));
  await listen("blast://notice", (e) => toast(e.payload.level, e.payload.message));
  await listen("blast://obs", (e) => obs.setObs(e.payload));
…
```

### [UI-17] Démarrage de l'interface

Initialise les modules, s'abonne, charge thèmes et voix, active le glisser-déposer puis vérifie les mises à jour.

`app/ui/js/main.js` ligne 47

```js
async function start() {
  for (const m of [theme, decks, playlist, cast, settings, voice, pads, jingles, obs, shortcuts, update, appearance, rotation, icecast, chat]) m.init();
  $("#themes-folder").addEventListener("click", async () => {
    try {
      const dir = await invoke("themes_dir");
      toast("info", `Dépose tes thèmes .toml dans : ${dir} — puis relance BLAST.`);
    } catch (e) {
      toast("error", String(e));
    }
  });
  setInterval(tickClock, 500);
  tickClock();
…
```

### [UI-18] Application d'un thème

Pose les variables CSS du thème et le mémorise.

`app/ui/js/theme.js` ligne 11

```js
function applyTheme(id) {
  const theme = themes.find((t) => t.id === id) || themes[0];
  if (!theme) return;
  current = theme;
  const root = document.documentElement;
  for (const section of ["colors", "meters"]) {
    for (const [k, v] of Object.entries(theme[section] || {})) {
      root.style.setProperty(`--${k.replaceAll("_", "-")}`, v);
    }
  }
  const fx = theme.effects || {};
  root.style.setProperty("--rainbow-speed", `${3 / Math.max(0.1, fx.rainbow_speed || 1)}s`);
…
```

### [UI-19] Effet Rainbow

Active ou coupe l'effet arc-en-ciel de la platine à l'antenne.

`app/ui/js/theme.js` ligne 30

```js
function setRainbow(on, persist = true) {
  view.rainbow = on;
  $("#rainbow-toggle").classList.toggle("on", on);
  if (persist) store.set("blast.rainbow", on ? "1" : "0");
  renderState();
}
```

### [UI-20] Chargement des thèmes

Remplit la liste et applique le thème mémorisé.

`app/ui/js/theme.js` ligne 38

```js
export async function loadThemes() {
  try {
    themes = await invoke("themes");
  } catch (_) {
    themes = [];
  }
  const sel = $("#theme-select");
  sel.innerHTML = "";
  for (const t of themes) {
    const o = document.createElement("option");
    o.value = t.id;
    o.textContent = (t.meta && t.meta.name) || t.id;
…
```

### [UI-21] Libellés d'état

Textes affichés dans la pastille d'état des platines.

`app/ui/js/decks.js` ligne 6

```js
const STATE_LABEL = {
  empty: "VIDE",
  loading: "CHARGEMENT",
  loaded: "PRÊTE",
  playing: "LECTURE",
  paused: "PAUSE",
  finished: "TERMINÉ",
};
```

### [UI-22] Affichage d'une platine

Titre, temps, progression, zone de fondu et boutons disponibles.

`app/ui/js/decks.js` ligne 20

```js
function renderDeck(id, d) {
  const el = decks[id];
  const st = S.state;
  el.classList.toggle("playing", d.state === "playing");
  el.classList.toggle("on-air", st.on_air === id);
  el.classList.toggle("rainbow", view.rainbow && st.on_air === id);
  $(".chip", el).textContent = STATE_LABEL[d.state] || d.state;
  $(".deck-title", el).textContent = d.title || "—";
  $(".deck-title", el).title = d.path || "";
  $(".deck-artist", el).innerHTML = d.artist ? "" : "&nbsp;";
  if (d.artist) $(".deck-artist", el).textContent = d.artist;
  const remaining = Math.max(0, d.duration - d.position);
…
```

### [UI-23] Vumètres

Vumètres L/R avec retombée, réduction du traitement et témoin CLIP.

`app/ui/js/decks.js` ligne 50

```js
function renderMeters(st) {
  const now = performance.now();
  meterHold.l = Math.max(dbWidth(st.peak[0]), meterHold.l * 0.85);
  meterHold.r = Math.max(dbWidth(st.peak[1]), meterHold.r * 0.85);
  $("#meter-l").style.width = `${(meterHold.l * 100).toFixed(1)}%`;
  $("#meter-r").style.width = `${(meterHold.r * 100).toFixed(1)}%`;
  meterHold.red = Math.max(Math.max(0, st.reduction || 0), meterHold.red * 0.8);
  $("#meter-red").style.width = `${Math.min(100, (meterHold.red / 12) * 100).toFixed(1)}%`;
  $("#red-v").textContent = meterHold.red >= 0.1 ? `-${meterHold.red.toFixed(1)} dB` : "0 dB";
  if (Math.max(st.peak[0], st.peak[1]) >= 0.999) meterHold.clipUntil = now + 1500;
  $("#clip").classList.toggle("on", now < meterHold.clipUntil);
}
```

### [UI-24] Affichage de l'état

Platines, ON AIR, crossfader, auto-mix, vumètres, sortie audio, micro et pads.

`app/ui/js/decks.js` ligne 64

```js

```

### [UI-25] Platine libre

Platine à utiliser pour un double-clic dans la playlist.

`app/ui/js/decks.js` ligne 103

```js
export function freeDeck() {
  const st = S.state;
  if (!st) return "A";
  const free = (d) => d.state !== "playing" && d.state !== "loading";
  const order = st.on_air === "A" ? ["B", "A"] : ["A", "B"];
  for (const id of order) {
    const d = st.decks[id === "A" ? 0 : 1];
    if (free(d) && (d.state === "empty" || d.state === "finished")) return id;
  }
  for (const id of order) {
    if (free(st.decks[id === "A" ? 0 : 1])) return id;
  }
…
```

### [UI-26] Affichage de la playlist

Lignes, titres en attente, fichiers illisibles et durée totale.

`app/ui/js/playlist.js` ligne 9

```js
let playlistView = null;
```

### [UI-27] Onglets playlist

Bascule entre PLAYLIST et HISTORIQUE.

`app/ui/js/playlist.js` ligne 73

```js
function showPlaylistTab(tab) {
  $$("[data-ptab]").forEach((t) => t.classList.toggle("active", t.dataset.ptab === tab));
  $$("#playlist [data-ppage]").forEach((el) => (el.hidden = el.dataset.ppage !== tab));
  if (tab === "playlist") $("#empty").classList.toggle("hidden", S.entries.length > 0);
  if (tab === "playlist") playlistView.refresh();
  if (tab === "history") historyView.refresh();
  document.dispatchEvent(new CustomEvent("blast:ptab", { detail: tab }));
}
```

### [UI-28] Heure d'un passage

Heure seule aujourd'hui, date et heure sinon.

`app/ui/js/playlist.js` ligne 83

```js
function clockTime(secs) {
  const d = new Date(secs * 1000);
  const hm = d.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  return d.toDateString() === new Date().toDateString() ? hm : `${d.toLocaleDateString("fr-FR", { day: "2-digit", month: "2-digit" })} ${hm}`;
}
```

### [UI-29] Affichage de l'historique

Reçoit les tampons et affiche celui du filtre choisi.

`app/ui/js/playlist.js` ligne 90

```js
export function setHistory(h) {
  history = h;
  renderHistory();
}
```

### [UI-30] Export de l'historique

Exporte la catégorie et la période choisies dans un fichier CSV.

`app/ui/js/playlist.js` ligne 158

```js
async function exportHistory() {
  const kind = $("#export-kind").value;
  const fromValue = $("#export-from").value;
  const toValue = $("#export-to").value;
  const from = fromValue ? Math.floor(new Date(`${fromValue}T00:00:00`).getTime() / 1000) : null;
  const to = toValue ? Math.floor(new Date(`${toValue}T00:00:00`).getTime() / 1000) + 86400 : null;
  closeExportMenu();
  const suffix = [kind === "all" ? "" : kind, fromValue, toValue].filter(Boolean).join("_") || day(new Date());
  const path = await T.dialog.save({ title: "Exporter l'historique", defaultPath: `blast-historique-${suffix}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
  if (!path) return;
  try {
    const n = await invoke("export_history", { path, kind, from, to });
…
```

### [UI-31] Cible de dépôt

Pad, platine, ligne (avant ou après) ou playlist sous le pointeur.

`app/ui/js/dnd.js` ligne 6

```js
function targetAt(x, y) {
  const el = document.elementFromPoint(x, y);
  if (!el) return null;
  const pad = el.closest(".pad:not(.jingle)");
  if (pad) return { kind: "pad", index: Number(pad.dataset.index), el: pad };
  const deck = el.closest(".deck");
  if (deck) return { kind: "deck", deck: deck.dataset.deck, el: deck };
  const row = el.closest(".row");
  if (row) {
    const r = row.getBoundingClientRect();
    const after = y > r.top + r.height / 2;
    return { kind: "row", index: Number(row.dataset.index) + (after ? 1 : 0), el: row, after };
…
```

### [UI-32] Surlignage de la cible

Met en évidence la cible de dépôt.

`app/ui/js/dnd.js` ligne 29

```js
function showTarget(t) {
  clearTargets();
  if (!t) return;
  if (t.kind === "deck" || t.kind === "playlist" || t.kind === "pad") t.el.classList.add("drop-target");
  if (t.kind === "row") t.el.classList.add(t.after ? "drop-after" : "drop-before");
}
```

### [UI-33] Dépôt d'une entrée

Charge sur une platine, affecte à un pad ou déplace dans la playlist.

`app/ui/js/dnd.js` ligne 37

```js
function dropEntry(t, id) {
  if (!t) return;
  if (t.kind === "deck") send({ type: "load_entry", id, deck: t.deck });
  else if (t.kind === "pad") {
    const e = S.entries.find((x) => x.id === id);
    if (e) assignPad(t.index, e.path, e.title);
  } else if (t.kind === "row" || t.kind === "playlist") send({ type: "move", id, to: t.index });
}
```

### [UI-34] Dépôt de fichiers

Fichiers venus de l'explorateur : pad, platine ou playlist à la position visée.

`app/ui/js/dnd.js` ligne 47

```js
function dropFiles(t, paths) {
  if (!paths.length) return;
  if (t && t.kind === "pad") assignPad(t.index, paths[0], "");
  else if (t && t.kind === "deck") send({ type: "load_paths", deck: t.deck, paths });
  else send({ type: "add_paths", paths, index: t && t.kind === "row" ? t.index : null });
}
```

### [UI-35] Glisser interne

Glisser-déposer des lignes de playlist au pointeur.

`app/ui/js/dnd.js` ligne 55

```js
function initInternal() {
  const ghost = $("#ghost");
  $("#rows").addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || e.target.closest("button")) return;
    const row = e.target.closest(".row");
    if (row) drag = { id: Number(row.dataset.id), row, x: e.clientX, y: e.clientY, active: false };
  });
  document.addEventListener("pointermove", (e) => {
    if (!drag) return;
    if (!drag.active) {
      if (Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 6) return;
      drag.active = true;
…
```

### [UI-36] Glisser depuis le système

Écoute les dépôts de fichiers de l'explorateur via Tauri.

`app/ui/js/dnd.js` ligne 88

```js
async function initOs() {
  const wv = T.webview && T.webview.getCurrentWebview ? T.webview.getCurrentWebview() : null;
  if (!wv || !wv.onDragDropEvent) return;
  await wv.onDragDropEvent((event) => {
    const p = event.payload;
    const dpr = window.devicePixelRatio || 1;
    const at = p.position ? targetAt(p.position.x / dpr, p.position.y / dpr) : null;
    if (p.type === "enter" || p.type === "over") {
      showTarget(at || { kind: "playlist", index: S.entries.length, el: $("#playlist") });
    } else {
      clearTargets();
      if (p.type === "drop") dropFiles(at, p.paths || []);
…
```

### [UI-37] Libellés des sorties

Textes des états de sortie.

`app/ui/js/cast.js` ligne 4

```js
export const STATUS_LABEL = { off: "Arrêtée", connecting: "Connexion…", live: "En direct", retrying: "Reconnexion", error: "Erreur" };
```

### [UI-38] Libellé du bouton DIFFUSER

EN DIRECT n/m, ERREUR, RECONNEXION ou CONNEXION.

`app/ui/js/cast.js` ligne 10

```js
function castLabel(states, live, total) {
  if (live > 0) return `EN DIRECT ${live}/${total}`;
  if (states.includes("error")) return "ERREUR";
  if (states.includes("retrying")) return "RECONNEXION…";
  return "CONNEXION…";
}
```

### [UI-39] Affichage de la diffusion

Bouton DIFFUSER, auditeurs, pic, infobulle et résumé de la barre d'état.

`app/ui/js/cast.js` ligne 31

```js
export function setCast(c) {
  cast = c;
  const btn = $("#cast-btn");
  const active = cast.outputs.filter((o) => o.enabled);
  const states = active.map(statusOf);
  const live = states.filter((s) => s === "live").length;
  btn.className = "cast-btn";
  if (cast.casting) btn.classList.add(castClass(states, live, active.length));
  $("#cast-label").textContent = cast.casting ? castLabel(states, live, active.length) : "DIFFUSER";
  renderCastTimer();
  const lis = $("#cast-listeners");
  const hasCount = cast.casting && cast.listeners != null;
…
```

### [UI-40] Durée de diffusion

Chronomètre depuis le passage en direct.

`app/ui/js/cast.js` ligne 113

```js
function renderCastTimer() {
  const t = $("#cast-time");
  const on = cast.casting && cast.since;
  t.classList.toggle("hidden", !on);
  if (on) {
    t.textContent = hms(Date.now() - cast.since);
    t.title = "Durée de diffusion";
  }
}
```

### [UI-41] Constantes des réglages

Débits, libellés des serveurs et sortie par défaut.

`app/ui/js/settings.js` ligne 6

```js
const BITRATES = [32, 40, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
```

### [UI-42] Liste des sorties

Sorties avec état, auditeurs et mot de passe manquant.

`app/ui/js/settings.js` ligne 16

```js
export function renderOutList() {
  const box = $("#out-items");
  box.innerHTML = "";
  for (const o of S.settings.outputs) {
    const co = cast.outputs.find((x) => x.id === o.id);
    const st = co ? co.status : null;
    const state = statusOf(co);
    const el = document.createElement("div");
    el.className = "out-item" + (o.id === selectedOut ? " active" : "") + (o.enabled ? "" : " disabled");
    el.innerHTML = `<div class="n"><span class="dot ${state}"></span><span></span></div><div class="t"></div><div class="s muted"></div>`;
    $(".n span:last-child", el).textContent = o.name;
    $(".t", el).textContent = `${KIND_LABEL[o.kind]} · ${o.host}:${o.port}${o.kind === "icecast" ? o.mount : ""} · ${o.bitrate} kb/s`;
…
```

### [UI-43] Sélection d'une sortie

Remplit le formulaire de la sortie choisie ou d'une nouvelle.

`app/ui/js/settings.js` ligne 44

```js
function selectOutput(id) {
  selectedOut = id;
  const form = $("#out-form");
  const o = S.settings.outputs.find((x) => x.id === id) || NEW_OUTPUT;
  fillForm(form, { ...o, mono: String(o.mono) });
  form.elements.password.value = "";
  form.elements.password.placeholder = o.has_password ? "•••••• enregistré (laisser vide pour garder)" : "mot de passe de la source";
  $("#out-delete").disabled = !o.id;
  setResult("#out-result", "", "");
  applyKindFields();
  renderOutList();
}
```

### [UI-44] Sortie saisie

Lit le formulaire en réglages de sortie et mot de passe.

`app/ui/js/settings.js` ligne 58

```js
function currentOutput() {
  const v = formValues($("#out-form"));
  const id = selectedOut && selectedOut !== "new" ? selectedOut : "";
  const out = {
    id, name: v.name, enabled: v.enabled, kind: v.kind, host: v.host.trim(), port: v.port,
    mount: v.mount, sid: v.sid || 1, user: v.user, bitrate: Number(v.bitrate), mono: v.mono === "true",
    station: v.station, genre: v.genre, description: v.description, url: v.url, public: v.public,
  };
  return { out, password: v.password || null };
}
```

### [UI-45] Liste de périphériques

Remplit une liste de sorties ou de micros avec l'entrée par défaut.

`app/ui/js/settings.js` ligne 70

```js
function fillDevices(sel, list, def, defLabel, value) {
  sel.innerHTML = "";
  const d = document.createElement("option");
  d.value = "";
  d.textContent = `${defLabel}${def ? " (" + def + ")" : ""}`;
  sel.appendChild(d);
  for (const name of list) {
    const o = document.createElement("option");
    o.value = name;
    o.textContent = name;
    sel.appendChild(o);
  }
…
```

### [UI-46] Affichage des réglages

Remplit tous les onglets à partir de la vue des réglages.

`app/ui/js/settings.js` ligne 93

```js
function applySettings(view, keepSelection = true) {
  S.settings = view;
  $("#store-info").textContent = `Mots de passe : ${view.secret_store}`;
  if (view.warning) toast("warn", view.warning);
  if (!keepSelection || !view.outputs.some((o) => o.id === selectedOut)) {
    selectedOut = view.outputs.length ? view.outputs[0].id : null;
  }
  if (selectedOut) selectOutput(selectedOut);
  else renderOutList();
  const chatForm = $("#chat-settings-form");
  fillForm(chatForm, view.chat);
  chatForm.elements.token.value = "";
…
```

### [UI-47] Enregistrement d'un formulaire

Enregistre, rafraîchit la vue et affiche le résultat.

`app/ui/js/settings.js` ligne 132

```js
async function saveForm(resultId, run) {
  try {
    const view = await run();
    if (view) applySettings(view);
    setResult(resultId, "ok", "Enregistré.");
  } catch (err) {
    setResult(resultId, "err", String(err));
  }
}
```

### [UI-48] Test d'une connexion

Affiche l'attente puis le résultat d'un test.

`app/ui/js/settings.js` ligne 143

```js
async function testForm(resultId, busy, run) {
  setResult(resultId, "busy", busy);
  try {
    setResult(resultId, "ok", await run());
  } catch (err) {
    setResult(resultId, "err", String(err));
  }
}
```

### [UI-49] Ouverture des réglages

Charge les réglages et ouvre la fenêtre, éventuellement sur un onglet.

`app/ui/js/settings.js` ligne 197

```js
export async function openSettings(tab) {
  try {
    applySettings(await invoke("get_settings"), false);
  } catch (e) {
    toast("error", String(e));
    return;
  }
  $("#settings").classList.remove("hidden");
  showTab(tab || $(".modal-tabs .tab.active").dataset.tab);
}
```

### [UI-50] Onglets des réglages

Affiche l'onglet choisi.

`app/ui/js/settings.js` ligne 215

```js
function showTab(tab) {
  $$(".tab").forEach((t) => t.classList.toggle("active", t.dataset.tab === tab));
  $$(".tab-page").forEach((p) => p.classList.toggle("hidden", p.dataset.page !== tab));
  if (tab === "about") renderAbout();
  document.dispatchEvent(new CustomEvent("blast:settings-tab", { detail: tab }));
}
```

### [UI-51] Suppression d'une sortie

Demande confirmation puis supprime.

`app/ui/js/settings.js` ligne 223

```js
async function deleteOutput() {
  if (!selectedOut || selectedOut === "new") return;
  const btn = $("#out-delete");
  if (btn.dataset.confirm !== selectedOut) {
    btn.dataset.confirm = selectedOut;
    btn.textContent = "Confirmer la suppression";
    setTimeout(() => {
      btn.dataset.confirm = "";
      btn.textContent = "Supprimer";
    }, 4000);
    return;
  }
…
```

### [UI-52] Curseurs de voix

Curseurs de Voice FX, clé de réglage et format d'affichage.

`app/ui/js/voice.js` ligne 3

```js
const SIGNED = (v) => `${v > 0 ? "+" : ""}${v} dB`;
```

### [UI-53] Affichage de Voice FX

Micro disponible, verrou, PARLER et vumètre micro.

`app/ui/js/voice.js` ligne 22

```js
export function renderVoice() {
  if (!S.state || !S.state.mic) return;
  const m = S.state.mic;
  const on = mic.latched && m.open;
  $("#voicefx").classList.toggle("disabled", !m.available);
  $("#mic-name").title = m.available ? `Micro : ${m.name || "par défaut"}` : m.error ? `Micro indisponible : ${m.error}` : "Aucun micro";
  $("#mic-toggle").classList.toggle("on", on);
  $("#mic-toggle").innerHTML = on ? `${ico("unlock")}<span>OUVERT</span>` : `${ico("lock")}<span>VERROU</span>`;
  $("#ptt").classList.toggle("active", m.open);
  $("#meter-mic").style.width = `${(dbWidth(m.peak) * 100).toFixed(1)}%`;
}
```

### [UI-54] Enregistrement différé

Enregistre les réglages de voix 250 ms après le dernier changement.

`app/ui/js/voice.js` ligne 35

```js
function saveSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    invoke("save_voice", { voice }).catch((e) => toast("error", String(e)));
  }, 250);
}
```

### [UI-55] Affichage des réglages de voix

Positionne curseurs et cases.

`app/ui/js/voice.js` ligne 49

```js
function applyVoice(v) {
  voice = { ...v };
  for (const [sel, key, label] of SLIDERS) {
    $(sel).value = voice[key];
    $(sel + "-v").textContent = label(voice[key]);
  }
  $("#eq-hpf").checked = !!voice.highpass;
  $("#mic-monitor").checked = !!voice.monitor;
  $("#duck-toggle").classList.toggle("on", !!voice.duck);
}
```

### [UI-56] Verrou du micro

Ouvre le micro en permanence ou le referme (bouton VERROU, touche M).

`app/ui/js/voice.js` ligne 61

```js
export function toggleLock() {
  mic.latched = !(mic.latched && S.state && S.state.mic.open);
  setMic(mic.latched);
}
```

### [UI-57] Parler (appui)

Ouvre le micro tant que PARLER ou Espace est maintenu.

`app/ui/js/voice.js` ligne 67

```js
export function pttDown() {
  if (mic.held) return;
  mic.held = true;
  setMic(true);
}
```

### [UI-58] Parler (relâche)

Referme le micro au relâchement, sauf s'il est verrouillé.

`app/ui/js/voice.js` ligne 74

```js
export function pttUp() {
  if (!mic.held) return;
  mic.held = false;
  if (!mic.latched) setMic(false);
}
```

### [UI-59] Chargement de Voice FX

Remplit les préréglages voix et antenne, les curseurs et le volume Sound FX.

`app/ui/js/voice.js` ligne 81

```js
export function applySettings(view) {
  fillSelect($("#voice-preset"), view.voice_presets, view.voice.preset);
  fillSelect($("#master-preset"), view.master_presets, view.audio.master_preset);
  applyVoice(view.voice);
  $("#sfx-vol").value = view.soundboard.volume_db;
}
```

### [UI-60] Pages de pads

12 pads par page, touches F1 à F12.

`app/ui/js/pads.js` ligne 3

```js
export const PADS_PER_PAGE = 12;
```

### [UI-61] Bouton de pad

Pad affecté, vide, en chargement ou en erreur, avec sa touche.

`app/ui/js/pads.js` ligne 17

```js
function padButton(index, k) {
  const p = padAt(index);
  const el = document.createElement("button");
  el.className = "pad" + (p ? "" : " vacant") + (p && !p.ready && !p.error ? " loading" : "") + (p && p.error ? " error" : "");
  el.dataset.index = index;
  el.style.setProperty("--pad-color", (p && p.color) || padColor(k));
  el.title = p ? `${p.name}\n${p.path}${p.error ? "\n⚠ " + p.error : ""}\nClic droit : modifier` : "Pad vide : clic pour choisir un son, ou glisse un fichier ici";
  const label = document.createElement("span");
  label.textContent = p ? p.name : "+";
  const key = document.createElement("span");
  key.className = "key";
  key.textContent = `F${k + 1}`;
…
```

### [UI-62] Affichage des pads

Page courante de la Sound FX.

`app/ui/js/pads.js` ligne 34

```js
function renderPads() {
  const pages = Math.max(1, Math.ceil(pads.count / PADS_PER_PAGE));
  page = Math.min(page, pages - 1);
  $("#pad-page").textContent = `${page + 1}/${pages}`;
  const box = $("#pads");
  box.innerHTML = "";
  for (let k = 0; k < PADS_PER_PAGE; k++) {
    const index = page * PADS_PER_PAGE + k;
    if (index >= pads.count) break;
    box.appendChild(padButton(index, k));
  }
  renderPadStates();
…
```

### [UI-63] Pads en lecture

Allume les pads qui jouent.

`app/ui/js/pads.js` ligne 54

```js
export function renderPadStates() {
  const mask = S.state ? BigInt(S.state.sfx || 0) : 0n;
  $$("#pads .pad").forEach((el) => {
    el.classList.toggle("playing", ((mask >> BigInt(el.dataset.index)) & 1n) === 1n);
  });
}
```

### [UI-64] Affectation d'un pad

Enregistre un son sur un pad.

`app/ui/js/pads.js` ligne 64

```js
export async function assignPad(index, path, name) {
  try {
    await invoke("save_pad", { pad: { index, name: name || "", path, color: "" } });
  } catch (e) {
    toast("error", String(e));
  }
}
```

### [UI-65] Déclenchement d'un pad

Joue le pad s'il est affecté.

`app/ui/js/pads.js` ligne 73

```js
export function playPad(index) {
  const known = !!padAt(index);
  if (known) send({ type: "pad_play", index });
  return known;
}
```

### [UI-66] Menu d'un pad

Renommer, changer de son ou vider un pad (clic droit).

`app/ui/js/pads.js` ligne 85

```js
function openPadMenu(e, index) {
  menuPad = index;
  const p = padAt(index);
  $("#pad-name").value = p ? p.name : "";
  $("#pad-rename").disabled = !p;
  $("#pad-clear").disabled = !p;
  const menu = $("#pad-menu");
  menu.classList.remove("hidden");
  const r = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(e.clientX, window.innerWidth - r.width - 8)}px`;
  menu.style.top = `${Math.min(e.clientY, window.innerHeight - r.height - 8)}px`;
  $("#pad-name").focus();
…
```

### [UI-67] Libellés OBS

Textes des états de connexion à OBS.

`app/ui/js/obs.js` ligne 3

```js
const OBS_STATUS = { off: "Déconnecté", connecting: "Connexion…", connected: "Connecté", retrying: "Reconnexion…", error: "Erreur" };
```

### [UI-68] Onglets Sound FX / OBS

Bascule entre la Sound FX et le panneau AURA · OBS.

`app/ui/js/obs.js` ligne 11

```js
export function showFx(page) {
  $$(".fx-tab[data-fx]").forEach((t) => t.classList.toggle("active", t.dataset.fx === page));
  $$("#soundfx [data-fxpage]").forEach((el) => (el.hidden = el.dataset.fxpage !== page));
  store.set("blast.fxtab", page);
}
```

### [UI-69] Chronomètres OBS

Durées de stream et d'enregistrement dans la barre du haut.

`app/ui/js/obs.js` ligne 18

```js
function renderTimers() {
  const now = Date.now();
  const live = $("#obs-live");
  const streaming = !!(obs.stream && obs.stream_since);
  live.classList.toggle("hidden", !streaming);
  if (streaming) $("span", live).textContent = hms(now - obs.stream_since);
  const rec = $("#obs-rec");
  const recording = !!(obs.record && obs.record_since);
  rec.classList.toggle("hidden", !recording);
  rec.classList.toggle("paused", !!obs.record_paused);
  if (recording && !obs.record_paused) $("span", rec).textContent = hms(now - obs.record_since);
  $("use", rec).setAttribute("href", obs.record_paused ? "#i-pause" : "#i-rec");
…
```

### [UI-70] Commandes OBS

Pastille, boutons stream, enregistrement et pause.

`app/ui/js/obs.js` ligne 33

```js
function renderControls(connected) {
  const st = obs.status;
  $("#obs-pill").className = `obs-pill ${st}`;
  $("#obs-pill").title = `OBS : ${OBS_STATUS[st] || st}${obs.detail ? " — " + obs.detail : ""}`;
  $(".fx-tab[data-fx='obs']").classList.toggle("obs-on", connected);
  $("#obs-dot").className = `dot ${connected ? "live" : st === "error" ? "error" : st === "off" ? "" : "connecting"}`;
  $("#obs-status-text").textContent = connected ? `Connecté — OBS ${obs.obs_version || ""}` : `${OBS_STATUS[st] || st}${obs.detail ? " : " + obs.detail : ""}`;
  $("#obs-status-text").title = obs.detail || "";
  $("#obs-connect").textContent = isIdle() ? "Connecter" : "Déconnecter";
  $$(".obs-grid .hw").forEach((b) => (b.disabled = !connected));
  $("#obs-pause").disabled = !connected || !obs.record;
  $("#obs-stream").classList.toggle("on-stream", !!obs.stream);
…
```

### [UI-71] Scènes OBS

Boutons de scène, scène active en évidence.

`app/ui/js/obs.js` ligne 53

```js
function renderScenes(connected) {
  const box = $("#obs-scenes");
  box.innerHTML = "";
  if (!connected || !obs.scenes.length) {
    const e = document.createElement("div");
    e.className = "obs-empty";
    e.textContent = connected ? "Aucune scène dans OBS" : "Connecte OBS pour afficher tes scènes";
    box.appendChild(e);
    return;
  }
  for (const name of obs.scenes) {
    const b = document.createElement("button");
…
```

### [UI-72] Sources audio OBS

Choix de la source, volume et muet.

`app/ui/js/obs.js` ligne 74

```js
function renderInputs(connected) {
  const sel = $("#obs-input");
  const names = obs.inputs.map((i) => i.name);
  if (!names.includes(input)) input = names[0] || null;
  sel.innerHTML = "";
  if (!names.length) {
    const o = document.createElement("option");
    o.textContent = connected ? "Aucune source audio" : "Source audio OBS";
    o.value = "";
    sel.appendChild(o);
  }
  for (const i of obs.inputs) {
…
```

### [UI-73] Raccourcis clavier

Espace pour parler, M pour le verrou, F1 à F12 pour les pads, Échap pour fermer.

`app/ui/js/shortcuts.js` ligne 10

```js
export function init() {
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closePadMenu();
    if (settingsOpen() || typing(e)) return;
    if (e.code === "Space") {
      e.preventDefault();
      if (!e.repeat) pttDown();
      return;
    }
    if (e.key === "m" || e.key === "M") {
      if (!e.repeat) toggleLock();
      return;
…
```

### [UI-74] Commandes de mise à jour

Commande à lancer selon le système.

`app/ui/js/update.js` ligne 3

```js
const UPGRADE = {
  linux: "git pull && ./scripts/install-linux.sh",
  windows: "git pull puis scripts\\install-windows.bat",
};
```

### [UI-75] Annonce d'une version

Notification avec « Voir la version » et « Ignorer cette version ».

`app/ui/js/update.js` ligne 19

```js
function announce(u) {
  const hint = UPGRADE[info && info.platform] || UPGRADE.linux;
  toast("update", `BLAST ${u.latest} est disponible — dans le dossier du projet : ${hint}`, [
    { label: "Voir la version", run: () => openLink(u.url) },
    { label: "Ignorer cette version", run: () => invoke("save_updates", { checkOnStart: null, skip: u.latest }) },
  ]);
}
```

### [UI-76] Vérification au démarrage

Vérifie si l'option est active et annonce une version plus récente.

`app/ui/js/update.js` ligne 28

```js
export async function checkOnStart() {
  try {
    info = await invoke("app_info");
    if (!info.updates.check_on_start) return;
    const u = await invoke("check_update", { automatic: true });
    if (u && u.newer) announce(u);
  } catch (_) {}
}
```

### [UI-77] Vérification manuelle

Bouton « Vérifier maintenant » de l'onglet À propos.

`app/ui/js/update.js` ligne 38

```js
async function checkNow() {
  setResult("#about-result", "busy", "Interrogation de GitHub…");
  try {
    const u = await invoke("check_update", { automatic: false });
    setResult("#about-result", u.newer ? "ok" : "", describe(u));
    $("#about-release").classList.toggle("hidden", !u.newer);
    $("#about-release").onclick = () => openLink(u.url);
  } catch (e) {
    setResult("#about-result", "err", String(e));
  }
}
```

### [UI-78] Onglet À propos

Version, système, commande de mise à jour, journal et dossier de réglages.

`app/ui/js/update.js` ligne 51

```js
export async function renderAbout() {
  try {
    info = await invoke("app_info");
  } catch (e) {
    toast("error", String(e));
    return;
  }
  $("#about-version").textContent = info.version;
  $("#about-platform").textContent = info.platform;
  $("#about-upgrade").textContent = UPGRADE[info.platform] || UPGRADE.linux;
  $("#about-log").textContent = info.log_path || "—";
  $("#about-config").textContent = info.config_dir || "—";
…
```

### [UI-79] Bouton de jingle

Nom, durée, prêt, en lecture ou en erreur.

`app/ui/js/jingles.js` ligne 7

```js
function jingleButton(j) {
  const el = document.createElement("button");
  el.className = "pad jingle" + (j.playing ? " playing" : "") + (!j.ready && !j.error ? " loading" : "") + (j.error ? " error" : "");
  el.dataset.index = j.index;
  el.title = `${j.name}\n${j.path}${j.error ? "\n⚠ " + j.error : ""}\nClic : lancer à l'antenne · reclic : arrêter`;
  const label = document.createElement("span");
  label.textContent = j.name;
  const dur = document.createElement("span");
  dur.className = "key";
  dur.textContent = j.duration != null ? fmt(j.duration) : "";
  el.append(label, dur);
  return el;
…
```

### [UI-80] Affichage de la box à jingles

Grille des jingles et nom du dossier.

`app/ui/js/jingles.js` ligne 22

```js
export function setJingles(v) {
  view = v;
  const box = $("#jingles");
  box.innerHTML = "";
  for (const j of view.items) box.appendChild(jingleButton(j));
  if (!view.items.length) {
    const e = document.createElement("div");
    e.className = "obs-empty";
    e.textContent = view.scanning
      ? "Lecture du dossier des jingles…"
      : view.dirs.length
        ? "Aucun son dans le dossier des jingles"
…
```

### [UI-81] Choix du dossier de jingles

Sélecteur de dossier du système.

`app/ui/js/jingles.js` ligne 44

```js
async function pickDir() {
  const picked = await T.dialog.open({ directory: true, multiple: false, title: "Dossier des jingles" });
  if (!picked) return;
  invoke("save_jingles", { dirs: [].concat(picked), volumeDb: null }).catch(errorOut);
}
```

### [UI-82] Ligne de l'historique

Construit une ligne du filtre choisi à la demande de la liste virtuelle, avec le type du passage.

`app/ui/js/playlist.js` ligne 96

```js
function historyRow(i) {
  const r = (history[filter] || [])[i];
  const el = document.createElement("div");
  const current = r.id === history.current;
  el.className = "hrow hrow-grid" + (current ? " current" : "");
  el.title = r.path;
  el.innerHTML = `<span class="t"></span><span class="k"><span class="kind-tag"></span></span><span class="ti"></span><span class="ar muted"></span><span class="d"></span><span class="a"><span class="air-dot"></span></span><span class="n"></span>`;
  $(".t", el).textContent = clockTime(r.started_at);
  $(".kind-tag", el).textContent = KIND_LABEL[r.kind] || r.kind;
  $(".kind-tag", el).classList.add(r.kind);
  $(".ti", el).textContent = r.title || "—";
  $(".ar", el).textContent = r.artist;
…
```

### [UI-83] Menu d'export

Ouvre le choix de la catégorie et de la période, le mois en cours par défaut.

`app/ui/js/playlist.js` ligne 140

```js
function openExportMenu(anchor) {
  const menu = $("#export-menu");
  $("#export-kind").value = filter;
  const now = new Date();
  if (!$("#export-from").value && !$("#export-to").value) {
    $("#export-from").value = day(new Date(now.getFullYear(), now.getMonth(), 1));
    $("#export-to").value = day(now);
  }
  menu.classList.remove("hidden");
  const r = anchor.getBoundingClientRect();
  const m = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(r.left, window.innerWidth - m.width - 8)}px`;
…
```

### [UI-84] Mots-clés saisis

Lit les mots-clés séparés par des virgules.

`app/ui/js/settings.js` ligne 190

```js
function categoryValues() {
  const v = formValues($("#categories-form"));
  const list = (text) => text.split(",").map((w) => w.trim()).filter(Boolean);
  return { jingle_keywords: list(v.jingle_keywords), pub_keywords: list(v.pub_keywords) };
}
```

### [UI-85] Couleur translucide

Convertit une couleur du thème en couleur avec transparence.

`app/ui/js/appearance.js` ligne 7

```js
function rgba(color, alpha) {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec((color || "").trim());
  if (!m) return color;
  let hex = m[1];
  if (hex.length === 3) hex = [...hex].map((c) => c + c).join("");
  const n = parseInt(hex, 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}
```

### [UI-86] Fond du verre

Image choisie, servie par le protocole de fichiers de Tauri, ou dégradé aux couleurs du thème.

`app/ui/js/appearance.js` ligne 24

```js
function backdrop(colors) {
  if (view.image) return `url("${T.core.convertFileSrc(view.image)}")`;
  const accent = colors.accent || "#FF1654";
  const second = colors.deck_b || "#888888";
  return `radial-gradient(circle at 18% 22%, ${rgba(accent, 0.55)}, transparent 45%), radial-gradient(circle at 82% 78%, ${rgba(second, 0.35)}, transparent 50%), linear-gradient(135deg, ${colors.bg || "#101010"}, ${colors.panel_alt || "#202020"})`;
}
```

### [UI-87] Application de l'apparence

Rend translucides les couleurs de fond du thème, règle flou et assombrissement, ou rétablit les couleurs pleines.

`app/ui/js/appearance.js` ligne 32

```js
export function applyAppearance() {
  const s = view.settings;
  const colors = themeColors();
  const root = document.documentElement;
  root.classList.toggle("glass", s.glass);
  root.style.setProperty("--glass-backdrop", backdrop(colors));
  root.style.setProperty("--glass-blur", `${s.blur}px`);
  root.style.setProperty("--glass-dim", String(s.dim));
  const light = luminance(colors.bg) > 0.5;
  root.style.setProperty("--glass-edge", light ? "rgba(255, 255, 255, 0.6)" : "rgba(255, 255, 255, 0.12)");
  root.style.setProperty("--glass-shadow", light ? "rgba(255, 255, 255, 0.7)" : "rgba(0, 0, 0, 0.55)");
  if (colors.text_muted && colors.text) {
…
```

### [UI-88] Réglage en direct

Applique chaque mouvement de curseur immédiatement et enregistre 300 ms après le dernier.

`app/ui/js/appearance.js` ligne 73

```js
function editLive() {
  const v = formValues($("#appearance-form"));
  view.settings = { ...view.settings, glass: v.glass, blur: Number(v.blur), opacity: Number(v.opacity), dim: Number(v.dim) };
  applyAppearance();
  renderForm();
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try {
      view = await invoke("save_appearance", { appearance: view.settings });
      setResult("#appearance-result", "ok", "Enregistré.");
    } catch (e) {
      setResult("#appearance-result", "err", String(e));
…
```

### [UI-89] Choix de l'image de fond

Sélecteur d'image du système, puis copie dans le dossier de BLAST.

`app/ui/js/appearance.js` ligne 90

```js
async function chooseImage() {
  const picked = await T.dialog.open({ multiple: false, directory: false, title: "Image de fond", filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "avif"] }] });
  if (!picked) return;
  setResult("#appearance-result", "busy", "Copie de l'image…");
  try {
    setView(await invoke("set_background", { path: picked }));
    setResult("#appearance-result", "ok", "Image appliquée.");
  } catch (e) {
    setResult("#appearance-result", "err", String(e));
  }
}
```

### [UI-90] Sous-pages des grilles

Bascule entre Bibliothèque, Catégories, Grilles et Planning.

`app/ui/js/rotation.js` ligne 22

```js
function showRotPage(page) {
  $$("[data-rot]").forEach((b) => b.classList.toggle("active", b.dataset.rot === page));
  $$("[data-rotpage]").forEach((p) => (p.hidden = p.dataset.rotpage !== page));
  if (page === "schedule") renderSchedule();
}
```

### [UI-91] Affichage de la bibliothèque

Dossiers, état du scan et nombre de titres.

`app/ui/js/rotation.js` ligne 29

```js
export function setLibrary(v) {
  library = v;
  const box = $("#lib-dirs");
  box.innerHTML = "";
  if (!library.dirs.length) {
    const e = document.createElement("div");
    e.className = "muted rot-empty";
    e.textContent = "Aucun dossier : ajoute le dossier de ta musique.";
    box.appendChild(e);
  }
  library.dirs.forEach((d, i) => {
    const row = document.createElement("div");
…
```

### [UI-92] Liste des catégories

Catégories avec leur couleur, et formulaire de celle choisie.

`app/ui/js/rotation.js` ligne 69

```js
function renderCategories() {
  const box = $("#cat-list");
  box.innerHTML = "";
  for (const c of rot.categories) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "rot-item" + (c.id === selCat ? " active" : "");
    row.append(dot(c.color), Object.assign(document.createElement("span"), { className: "rot-name", textContent: c.name || "Sans nom" }));
    row.addEventListener("click", () => {
      selCat = c.id;
      renderCategories();
    });
…
```

### [UI-93] Saisie d'une catégorie

Lit le formulaire en règles de catégorie, à chaque frappe.

`app/ui/js/rotation.js` ligne 105

```js
function readCategory() {
  const cat = rot.categories.find((c) => c.id === selCat);
  if (!cat) return;
  const form = $("#cat-form");
  const v = formValues(form);
  const raw = (n) => num(form.elements[n].value);
  cat.name = v.name.trim();
  cat.color = v.color;
  cat.rule = {
    kinds: ["morceau", "jingle", "pub"].filter((k) => v[`k_${k}`]),
    genres: words(v.genres),
    year_min: raw("year_min"),
…
```

### [UI-94] Liste des grilles

Grilles avec leur couleur et leur nombre de cases.

`app/ui/js/rotation.js` ligne 146

```js
function renderClocks() {
  const box = $("#clock-list");
  box.innerHTML = "";
  for (const k of rot.clocks) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "rot-item" + (k.id === selClock ? " active" : "");
    row.append(dot(k.color), Object.assign(document.createElement("span"), { className: "rot-name", textContent: `${k.name || "Sans nom"} · ${k.slots.length} case(s)` }));
    row.addEventListener("click", () => {
      selClock = k.id;
      renderClocks();
    });
…
```

### [UI-95] Cases d'une grille

Catégorie de chaque case, montée, descente et retrait.

`app/ui/js/rotation.js` ligne 168

```js
function renderSlots(clock) {
  const box = $("#clock-slots");
  box.innerHTML = "";
  if (!rot.categories.length) {
    box.textContent = "Crée d'abord une catégorie.";
    return;
  }
  clock.slots.forEach((id, i) => {
    const row = document.createElement("div");
    row.className = "clock-slot";
    const n = Object.assign(document.createElement("span"), { className: "slot-num", textContent: String(i + 1) });
    const sel = document.createElement("select");
…
```

### [UI-96] Planning

Palette des grilles et des émissions (hachurées) et tableau des 7 jours × 24 heures, à peindre au clic ou au glisser.

`app/ui/js/rotation.js` ligne 247

```js
function renderSchedule() {
  const pal = $("#sched-palette");
  pal.innerHTML = "";
  const choices = [{ id: null, name: "Effacer", color: "transparent" }, ...rot.clocks, ...rot.programs];
  for (const k of choices) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "chip-btn sched-chip" + (paint === k.id ? " active" : "") + (isProgram(k.id) ? " prog" : "");
    const d = dot(k.id ? k.color : "transparent");
    if (isProgram(k.id)) {
      d.style.background = "";
      d.style.setProperty("--cell-color", k.color);
…
```

### [UI-97] Chargement des grilles

Copie de travail des grilles à l'ouverture des réglages.

`app/ui/js/rotation.js` ligne 306

```js
export function applySettingsView(view) {
  rot = JSON.parse(JSON.stringify(view.rotation));
  if (!rot.categories.some((c) => c.id === selCat)) selCat = rot.categories[0] ? rot.categories[0].id : null;
  if (!rot.clocks.some((k) => k.id === selClock)) selClock = rot.clocks[0] ? rot.clocks[0].id : null;
  rot.programs = rot.programs || [];
  if (!rot.programs.some((p) => p.id === selProg)) selProg = rot.programs[0] ? rot.programs[0].id : null;
  if (paint && !scheduled(paint)) paint = null;
  renderCategories();
  renderClocks();
  renderPrograms();
  renderSchedule();
}
```

### [UI-98] Enregistrement des grilles

Envoie la copie de travail ; une erreur de validation s'affiche sans rien perdre.

`app/ui/js/rotation.js` ligne 320

```js
async function save() {
  try {
    const view = await invoke("save_rotation", { rotation: rot });
    applySettingsView(view);
    setResult("#rotation-result", "ok", "Grilles enregistrées.");
  } catch (e) {
    setResult("#rotation-result", "err", String(e));
  }
}
```

### [UI-99] Grille dans le mixeur

Affiche le nom et la couleur de la grille ou de l'émission (précédée de ▶) de l'heure à côté du titre MIX.

`app/ui/js/decks.js` ligne 65

```js
function renderClock(clock) {
  const title = $(".mixer-title");
  const key = clock ? `${clock.id}|${clock.name}|${clock.color}|${clock.program}` : "";
  if (title.dataset.clock === key) return;
  title.dataset.clock = key;
  title.textContent = "MIX";
  title.title = clock ? `${clock.program ? "Émission" : "Grille"} de l'heure : ${clock.name}` : "Aucune grille pour cette heure : l'auto-mix suit la playlist";
  if (!clock) return;
  const tag = document.createElement("span");
  tag.className = "clock-tag";
  tag.style.setProperty("--clock-color", clock.color || "var(--accent)");
  tag.textContent = clock.program ? `\u25B6 ${clock.name}` : clock.name;
…
```

### [UI-100] Menu Mélanger

Mélanger, mélanger en espaçant les artistes, retirer les doublons.

`app/ui/js/playlist.js` ligne 177

```js
function openShuffleMenu(anchor) {
  const menu = $("#shuffle-menu");
  menu.classList.remove("hidden");
  const r = anchor.getBoundingClientRect();
  const m = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(r.left, window.innerWidth - m.width - 8)}px`;
  menu.style.top = `${r.bottom + 6}px`;
}
```

### [UI-101] Marge de la liste virtuelle

Nombre de lignes rendues en plus au-dessus et au-dessous de la zone visible.

`app/ui/js/vlist.js` ligne 1

```js
const OVERSCAN = 8;
```

### [UI-102] Liste virtuelle

Ne crée que les lignes visibles d'une longue liste ; un espaceur donne la hauteur totale et le rendu suit le défilement.

`app/ui/js/vlist.js` ligne 4

```js
export function virtualList(container, rowHeight, renderRow, renderEmpty) {
  const spacer = document.createElement("div");
  spacer.className = "vspacer";
  container.replaceChildren(spacer);
  let count = 0;
  let pending = false;

  function draw() {
    pending = false;
    spacer.style.height = `${count * rowHeight}px`;
    if (count === 0) {
      spacer.replaceChildren(...(renderEmpty ? [renderEmpty()] : []));
…
```

### [UI-103] Ligne de la playlist

Construit une ligne de la playlist à la demande de la liste virtuelle.

`app/ui/js/playlist.js` ligne 13

```js
function playlistRow(i) {
  const e = S.entries[i];
  const row = document.createElement("div");
  row.className = "row row-grid" + (e.error ? " error" : "");
  row.dataset.id = e.id;
  row.dataset.index = i;
  row.title = e.error ? `${e.path}\n${e.error}` : e.path;
  row.innerHTML = `
    <span class="num">${i + 1}</span>
    <span class="title"></span>
    <span class="artist muted"></span>
    <span class="dur">${e.duration != null ? fmt(e.duration) : "--:--"}</span>
…
```

### [UI-104] Motifs d'attente

Applique les motifs d'attente reçus aux entrées et redessine seulement les lignes visibles.

`app/ui/js/playlist.js` ligne 66

```js
export function setBlocked(list) {
  const reasons = new Map(list);
  for (const e of S.entries) e.blocked = reasons.get(e.id) || null;
  playlistView.refresh();
}
```

### [UI-105] Enregistrer la playlist

Demande l'emplacement du fichier M3U, nommé par défaut avec la date du jour.

`app/ui/js/playlist.js` ligne 189

```js
async function saveM3u() {
  if (!S.entries.length) return toast("warn", "La playlist est vide.");
  const day = new Date().toISOString().slice(0, 10);
  const path = await T.dialog.save({
    title: "Enregistrer la playlist",
    defaultPath: `blast-playlist-${day}.m3u8`,
    filters: [{ name: "Playlist M3U", extensions: ["m3u8", "m3u"] }],
  });
  if (path) send({ type: "save_playlist", path });
}
```

### [UI-106] Liste des émissions

Liste des émissions et formulaire de l'émission choisie.

`app/ui/js/rotation.js` ligne 209

```js
function renderPrograms() {
  const box = $("#prog-list");
  box.innerHTML = "";
  for (const p of rot.programs) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "rot-item" + (p.id === selProg ? " active" : "");
    row.append(dot(p.color), Object.assign(document.createElement("span"), { className: "rot-name", textContent: p.name || "Sans nom" }));
    row.addEventListener("click", () => {
      selProg = p.id;
      renderPrograms();
    });
…
```

### [UI-107] Lecture d'une émission

Recopie le nom, la couleur et le chemin du M3U depuis le formulaire.

`app/ui/js/rotation.js` ligne 229

```js
function readProgram() {
  const prog = rot.programs.find((p) => p.id === selProg);
  if (!prog) return;
  const v = formValues($("#prog-form"));
  prog.name = v.name.trim();
  prog.color = v.color;
  prog.path = v.path.trim();
  const item = $("#prog-list .rot-item.active");
  if (item) {
    $(".rot-name", item).textContent = prog.name || "Sans nom";
    $(".rot-dot", item).style.background = prog.color;
  }
…
```

### [UI-108] État du module Icecast

Serveur choisi, onglet, états et historiques par serveur, configuration chargée et brouillon en cours de modification.

`app/ui/js/icecast.js` ligne 4

```js
const I = {
  view: null,
  sel: null,
  draftServer: null,
  tab: "dash",
  status: {},
  history: {},
  rate: {},
  peak: {},
  conf: null,
  confError: null,
  draft: null,
…
```

### [UI-109] Outils de format Icecast

Échappement HTML, octets, durées, accès aux champs par chemin (auth.admin_password).

`app/ui/js/icecast.js` ligne 31

```js
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
```

### [UI-110] Liste des serveurs

Un bouton par serveur avec son état et ses auditeurs.

`app/ui/js/icecast.js` ligne 72

```js
function renderServers() {
  const box = $("#ice-servers");
  box.innerHTML = "";
  for (const s of I.view.servers) {
    const status = I.status[s.id];
    const b = document.createElement("button");
    b.className = "ice-server" + (s.id === I.sel ? " active" : "");
    const n = status && status.stats ? status.stats.listeners : null;
    b.innerHTML = `<span class="dot ${stateOf(status) === "running" ? "connecting" : stateOf(status)}"></span><span class="n"></span><span class="c"></span><span class="t"></span>`;
    $(".n", b).textContent = s.name;
    $(".c", b).textContent = n != null ? String(n) : "";
    $(".c", b).title = "auditeurs";
…
```

### [UI-111] Choix d'un serveur

Charge sa configuration si le serveur est local puis son état ; prévient si un brouillon est abandonné.

`app/ui/js/icecast.js` ligne 127

```js
async function selectServer(id) {
  if (I.draft && dirty() && id !== I.sel) toast("warn", "Les modifications non enregistrées de la configuration précédente sont abandonnées.");
  I.sel = id;
  I.conf = null;
  I.draft = null;
  I.confError = null;
  I.mount = null;
  renderServers();
  renderHead();
  const s = server();
  const tab = s && s.remote && ["config", "mounts", "log"].includes(I.tab) ? "dash" : I.tab;
  showITab(tab === "profile" && I.sel !== "new" ? "dash" : tab);
…
```

### [UI-112] Onglets Icecast

Affiche une page ; les onglets réservés aux serveurs locaux sont désactivés pour un serveur distant ou non enregistré.

`app/ui/js/icecast.js` ligne 144

```js
function showITab(tab) {
  const s = server();
  const isNew = I.sel === "new";
  I.tab = tab;
  $$(".ice-tabs .fx-tab").forEach((t) => {
    const off = isNew ? t.dataset.itab !== "profile" : s && s.remote && t.hasAttribute("data-local");
    t.disabled = !!off;
    t.title = off ? (isNew ? "Enregistre d'abord la connexion" : "Disponible pour un serveur local (SSH à venir pour un VPS)") : "";
    t.classList.toggle("active", t.dataset.itab === tab);
  });
  $$(".ice-page").forEach((p) => p.classList.toggle("hidden", p.dataset.ipage !== tab));
  if (tab === "profile") fillProfile();
…
```

### [UI-113] Actualisation de l'état

Interroge ice_status, enregistre l'historique des auditeurs et le débit sortant calculé sur 30 s.

`app/ui/js/icecast.js` ligne 188

```js
async function refreshStatus(id = I.sel) {
  if (!id || id === "new") return;
  try {
    const s = await invoke("ice_status", { id });
    I.status[id] = s;
    record(id, s);
  } catch (e) {
    I.status[id] = { error: String(e), running: false, reachable: false };
  }
  if (!isOpen()) return;
  renderServers();
  if (id === I.sel) {
…
```

### [UI-114] En-tête Icecast

Pastille d'état, adresse, version, origine du processus et boutons selon l'état.

`app/ui/js/icecast.js` ligne 166

```js
function renderHead() {
  const s = server();
  if (!s) return;
  const status = st();
  const state = stateOf(status);
  $("#ice-dot").className = "dot " + (state === "live" ? "live" : state === "running" ? "running" : status && status.error && s.remote ? "error" : "");
  $("#ice-name").textContent = s.name || "Nouveau serveur";
  const parts = [`${s.remote ? "Distant" : "Local"} · ${s.https ? "https" : "http"}://${s.host}:${s.port}`];
  if (status && status.server_id) parts.push(status.server_id);
  if (status && status.running && !s.remote) parts.push(status.owned ? "lancé par BLAST" : "processus repris");
  $("#ice-sub").textContent = I.sel === "new" ? "Configure la connexion puis enregistre." : parts.join(" · ");
  const running = !!(status && status.running);
…
```

### [UI-115] Tableau de bord Icecast

Bandeau d'information, tuiles et tableau des mountpoints (actifs et déclarés en attente de source).

`app/ui/js/icecast.js` ligne 222

```js
function renderDash() {
  const s = st();
  const banner = $("#ice-banner");
  const srv = server();
  let note = s && s.error ? s.error : "";
  let level = "";
  if (s && !s.running && !s.reachable && srv && !srv.remote) note = "Icecast est arrêté. Clique sur Démarrer.";
  if (s && s.reachable && !s.running && srv && !srv.remote) note = (note ? note + " " : "") + "Icecast répond mais n'a pas été lancé par BLAST (service Windows ou autre fenêtre) : Démarrer et Arrêter sont indisponibles.";
  if (s && s.error && srv && srv.remote && !s.reachable) level = "error";
  banner.textContent = note;
  banner.className = "ice-banner" + (level ? " " + level : "") + (note ? "" : " hidden");
  const stats = s && s.stats;
…
```

### [UI-116] Graphique des auditeurs

Courbe en escalier des 10 dernières minutes, aux couleurs du thème.

`app/ui/js/icecast.js` ligne 305

```js
function drawChart() {
  const c = $("#ice-chart");
  const w = c.clientWidth;
  const h = c.clientHeight;
  if (!w || !h) return;
  const dpr = window.devicePixelRatio || 1;
  c.width = Math.round(w * dpr);
  c.height = Math.round(h * dpr);
  const g = c.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  const css = getComputedStyle(document.documentElement);
…
```

### [UI-117] Actions sur un mountpoint

Menu flottant : voir les auditeurs, changer le titre, déplacer les auditeurs, couper la source.

`app/ui/js/icecast.js` ligne 360

```js
function openMenu(anchor, html, bind) {
  const m = $("#ice-menu");
  m.innerHTML = html;
  m.classList.remove("hidden");
  const r = anchor.getBoundingClientRect();
  m.style.left = `${Math.max(8, Math.min(window.innerWidth - 296, r.right - 280))}px`;
  m.style.top = `${Math.min(window.innerHeight - m.offsetHeight - 8, r.bottom + 6)}px`;
  bind(m);
}
```

### [UI-118] Chargement de la configuration

Lit icecast.xml et prépare un brouillon modifiable.

`app/ui/js/icecast.js` ligne 426

```js
async function loadConfig() {
  try {
    const v = await invoke("ice_get_config", { id: I.sel });
    applyConfigView(v);
  } catch (e) {
    I.conf = null;
    I.draft = null;
    I.confError = String(e);
  }
  if (I.tab === "config") fillConfig();
  if (I.tab === "mounts") renderMounts();
  renderSavebar();
…
```

### [UI-119] Formulaire de configuration

Remplit les champs data-k depuis le brouillon et les désactive si le fichier est en lecture seule.

`app/ui/js/icecast.js` ligne 458

```js
function fillConfig() {
  const noconf = !I.draft;
  $("#ice-noconf").classList.toggle("hidden", !noconf);
  $("#ice-config-form").hidden = noconf;
  $("#ice-file").classList.toggle("hidden", noconf);
  if (noconf) {
    const s = server();
    $("#ice-noconf-text").textContent = I.confError || "Aucun fichier de configuration n'est associé à ce serveur.";
    $("#ice-create").classList.toggle("hidden", !s || s.remote || !!(s.config_path && s.managed));
    return;
  }
  const v = I.confView;
…
```

### [UI-120] Ports d'écoute

Une ligne par port ; le dernier port ne peut pas être retiré.

`app/ui/js/icecast.js` ligne 496

```js
function renderSockets() {
  const box = $("#ice-sockets");
  box.innerHTML = "";
  const ro = I.confView && !I.confView.writable;
  I.draft.sockets.forEach((s, i) => {
    const row = document.createElement("div");
    row.className = "ice-row";
    row.innerHTML = `
      <label>Port<input type="number" min="1" max="65535" data-s="port"></label>
      <label>Adresse d'écoute<input data-s="bind_address" placeholder="toutes (0.0.0.0)"></label>
      <label title="Pour les vieux logiciels qui ne parlent que le protocole Shoutcast : ils se connectent au port suivant et arrivent sur ce point de montage">Compatibilité Shoutcast (facultatif)<input data-s="shoutcast_mount" placeholder="laisser vide en général"></label>
      <label class="check"><input type="checkbox" data-s="ssl"> HTTPS</label>
…
```

### [UI-121] Mountpoints

Liste et formulaire des mountpoints du brouillon.

`app/ui/js/icecast.js` ligne 532

```js
function renderMounts() {
  const list = $("#ice-mount-list");
  list.innerHTML = "";
  const has = !!I.draft;
  $("#ice-mount-add").disabled = !has || (I.confView && !I.confView.writable);
  if (!has) {
    $("#ice-mount-empty").textContent = I.confError || "Associe d'abord un fichier de configuration (onglet CONFIGURATION).";
    $("#ice-mount-empty").classList.remove("hidden");
    $("#ice-mount-body").classList.add("hidden");
    return;
  }
  const live = new Map(((st() && st().stats && st().stats.mounts) || []).map((m) => [m.mount, m]));
…
```

### [UI-122] Enregistrement de la configuration

Envoie le brouillon, redémarre si demandé et recharge les profils.

`app/ui/js/icecast.js` ligne 599

```js
async function saveConfig(restart) {
  if (!I.draft) return;
  setResult("#ice-save-result", "busy", restart ? "Enregistrement et redémarrage…" : "Enregistrement…");
  try {
    const v = await invoke("ice_save_config", { id: I.sel, config: I.draft, restart });
    applyConfigView(v);
    setResult("#ice-save-result", "", "");
    I.view = await invoke("ice_list");
    renderServers();
    showITab(I.tab);
    setTimeout(() => refreshStatus(), 800);
  } catch (e) {
…
```

### [UI-123] Liste des auditeurs

Auditeurs du mountpoint choisi, du plus ancien au plus récent, avec déconnexion confirmée.

`app/ui/js/icecast.js` ligne 616

```js
async function loadListeners() {
  if (I.tab !== "listeners" || !I.sel || I.sel === "new") return;
  const sel = $("#ice-lis-mount");
  const mounts = ((st() && st().stats && st().stats.mounts) || []).filter((m) => m.connected).map((m) => m.mount);
  const want = sel.dataset.want || sel.value;
  sel.dataset.want = "";
  sel.innerHTML = mounts.map((m) => `<option>${esc(m)}</option>`).join("");
  if (mounts.includes(want)) sel.value = want;
  const tb = $("#ice-lis-table tbody");
  if (!mounts.length) {
    tb.innerHTML = "";
    $("#ice-lis-none").textContent = "Aucun mountpoint ne diffuse en ce moment.";
…
```

### [UI-124] Journal

Fin du journal choisi, erreurs et avertissements colorés, défilement suivi.

`app/ui/js/icecast.js` ligne 658

```js
async function loadLog() {
  if (I.tab !== "log" || !I.sel || I.sel === "new") return;
  const pre = $("#ice-log");
  const atEnd = pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 30;
  try {
    const text = await invoke("ice_log", { id: I.sel, which: I.logKind, lines: 500 });
    pre.innerHTML = text
      .split("\n")
      .map((l) => {
        const cls = / EROR /.test(l) ? "e" : / WARN /.test(l) ? "w" : "";
        return cls ? `<span class="${cls}">${esc(l)}</span>` : esc(l);
      })
…
```

### [UI-125] Profil du serveur

Formulaire de connexion, choix du dossier et du fichier, détection de l'exécutable, avertissement HTTP pour un VPS.

`app/ui/js/icecast.js` ligne 678

```js
function fillProfile() {
  const s = server();
  if (!s) return;
  const f = $("#ice-profile-form").elements;
  f.name.value = s.name;
  f.remote.value = String(!!s.remote);
  f.install_dir.value = s.install_dir || "";
  f.config_path.value = s.config_path || "";
  f.host.value = s.host;
  f.port.value = s.port;
  f.admin_user.value = s.admin_user;
  f.https.checked = !!s.https;
…
```

### [UI-126] Pastille ICECAST *(retiré)*

Pastille de la barre du haut, retirée : le module vit dans l'onglet Serveur Icecast des Réglages.

### [UI-127] Ouverture de l'onglet Icecast

À l'affichage de l'onglet, charge les profils, choisit un serveur et lance les actualisations ; elles s'arrêtent quand on change d'onglet ou qu'on ferme les Réglages.

`app/ui/js/icecast.js` ligne 812

```js
export const isOpen = () => settingsOpen() && !$('[data-page="icecast"]').classList.contains("hidden");
```

### [UI-128] Démarrer, arrêter, redémarrer

Commandes du serveur local, avec retour dans une notification.

`app/ui/js/icecast.js` ligne 765

```js
async function power(action) {
  I.busy = true;
  renderHead();
  try {
    toast("info", await invoke("ice_power", { id: I.sel, action }));
  } catch (e) {
    toast("error", String(e));
    if (I.tab !== "log") {
      I.logKind = "console";
      $$("[data-ilog]").forEach((b) => b.classList.toggle("active", b.dataset.ilog === "console"));
    }
  }
…
```

### [UI-129] Accueil Icecast

Installations détectées pas encore ajoutées.

`app/ui/js/icecast.js` ligne 92

```js
function renderWelcome() {
  const box = $("#ice-detected");
  box.innerHTML = "";
  const known = new Set(I.view.servers.map((s) => s.exe).filter(Boolean));
  const list = I.view.detected.filter((d) => !known.has(d.exe));
  if (!list.length) {
    box.innerHTML = `<div class="muted">Aucune autre installation détectée dans les emplacements habituels. Choisis le dossier à la main.</div>`;
  }
  for (const d of list) {
    const row = document.createElement("div");
    row.className = "ice-found";
    row.innerHTML = `<code></code><span class="muted"></span><button class="btn accent small">Ajouter</button>`;
…
```

### [UI-130] Sortie BLAST vers /live

Depuis le tableau de bord sans flux en cours, crée en un clic la sortie Icecast vers /live avec le mot de passe des sources.

`app/ui/js/icecast.js` ligne 802

```js
async function quickLink() {
  try {
    const msg = await invoke("ice_link_output", { id: I.sel, mount: "/live" });
    toast("info", msg, [{ label: "Voir la sortie", run: () => openSettings("cast") }]);
  } catch (e) {
    toast("error", String(e));
  }
}
```

### [UI-131] Bouton DIFFUSER

Lance ou arrête la diffusion ; sans sortie configurée, prévient au lieu d'ouvrir les Réglages.

`app/ui/js/cast.js` ligne 59

```js
function toggleCast() {
  if (!cast.outputs.length) {
    toast("warn", "Aucune sortie de diffusion : ajoute-en une dans Réglages → Diffusion.");
    return;
  }
  send({ type: cast.casting ? "cast_stop" : "cast_start" });
}
```

### [UI-132] Détail de la diffusion

Panneau ouvert par le « + » du bouton DIFFUSER : état et durée, titre en cours, état et auditeurs de chaque sortie, total et pic, démarrer ou arrêter, accès aux sorties.

`app/ui/js/cast.js` ligne 70

```js
function renderCastPanel() {
  if (!panelOpen()) return;
  const active = cast.outputs.filter((o) => o.enabled);
  const live = active.filter((o) => statusOf(o) === "live").length;
  $("#cp-state").textContent = cast.casting ? `En direct sur ${live}/${active.length} sortie${active.length > 1 ? "s" : ""}` : "Diffusion arrêtée";
  $("#cp-time").textContent = cast.casting && cast.since ? hms(Date.now() - cast.since) : "";
  $("#cp-song").textContent = cast.casting && cast.song ? `♪ ${cast.song}` : "";
  const box = $("#cp-outputs");
  box.innerHTML = "";
  if (!cast.outputs.length) box.innerHTML = `<div class="muted">Aucune sortie configurée.</div>`;
  for (const o of cast.outputs) {
    const row = document.createElement("div");
…
```

### [UI-133] Affichage du chat

Messages gardés, badges affichés et couleurs des viewers sans couleur choisie.

`app/ui/js/chat.js` ligne 4

```js
const KEEP = 300;
```

### [UI-134] Couleur d'un viewer

Couleur choisie sur Twitch, sinon une couleur stable tirée de son identifiant.

`app/ui/js/chat.js` ligne 16

```js
function colorOf(m) {
  if (m.color) return m.color;
  let h = 0;
  for (const c of m.login || m.author) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return PALETTE[h % PALETTE.length];
}
```

### [UI-135] Ligne du chat

Heure, badges, nom en couleur et texte ; commandes surlignées, actions en italique, annonces encadrées.

`app/ui/js/chat.js` ligne 28

```js
function row(e) {
  const el = document.createElement("div");
  if (e.kind === "notice") {
    el.className = "chat-row notice";
    el.textContent = e.text;
    return el;
  }
  el.className = "chat-row" + (e.own ? " own" : "") + (e.action ? " action" : "");
  el.dataset.id = e.id;
  const time = document.createElement("span");
  time.className = "chat-time";
  time.textContent = hhmm(e.at_ms);
…
```

### [UI-136] File des demandes

Cartes des demandes : viewer, heure, titre trouvé ou recherche introuvable, Jouer ensuite et Refuser.

`app/ui/js/chat.js` ligne 92

```js
function renderRequests() {
  const list = $("#chat-req-list");
  list.innerHTML = "";
  $("#chat-req-count").textContent = view.requests.length ? `${view.requests.length}` : "";
  if (!view.requests.length) {
    list.innerHTML = `<div class="muted chat-req-empty">Aucune demande. Les viewers écrivent <code>!request Artiste - Titre</code>.</div>`;
    return;
  }
  for (const r of view.requests) {
    const card = document.createElement("div");
    card.className = "chat-req" + (r.path ? "" : " missing");
    const head = document.createElement("div");
…
```

### [UI-137] État du chat

Pastille et texte d'état, bouton Connecter/Déconnecter, champ d'envoi actif seulement si l'écriture est possible.

`app/ui/js/chat.js` ligne 128

```js
function renderStatus() {
  const dot = $("#chat-dot");
  dot.className = "dot " + (view.connected ? "live" : view.active ? "connecting" : "");
  const channel = view.channel ? `#${view.channel}` : "Twitch";
  $("#chat-status").textContent = view.connected ? `${channel}${view.writable ? "" : " · lecture seule"}` : view.active ? `${channel} · ${view.detail}` : channel;
  $("#chat-status").title = view.detail || "";
  const btn = $("#chat-connect");
  btn.textContent = view.active ? "Déconnecter" : "Connecter";
  btn.classList.toggle("accent", !view.active);
  const input = $("#chat-input");
  const can = view.connected && view.writable;
  input.disabled = !can;
…
```

### [UI-138] Valeurs du chat

Lit le formulaire des réglages du chat, avec le jeton saisi ou l'oubli du jeton.

`app/ui/js/settings.js` ligne 153

```js
function chatValues(forget) {
  const v = formValues($("#chat-settings-form"));
  return {
    chat: {
      channel: v.channel.trim(),
      bot_login: v.bot_login.trim(),
      autoconnect: v.autoconnect,
      announce: v.announce,
      cmd_np: v.cmd_np,
      cmd_request: v.cmd_request,
      cmd_sfx: v.cmd_sfx,
      request_limit: Math.max(1, Number(v.request_limit) || 2),
…
```


## STY

Interface : feuille de style

### [STY-01] Variables, base et utilitaires

Valeurs par défaut des couleurs (remplacées par le thème), polices, champs et icônes.

`app/ui/style.css` ligne 1

```css
:root {
  --bg: #0A0A0A;
  --panel: #141414;
  --panel-alt: #1E1E1E;
  --border: #2A2A2A;
  --text: #FFFFFF;
  --text-muted: #9A9A9A;
  --accent: #FF1654;
  --accent-hover: #FF4A7A;
  --deck-a: #FF1654;
  --deck-b: #FFFFFF;
  --waveform: #E0E0E0;
…
```

### [STY-02] Boutons

Styles communs des boutons et variantes.

`app/ui/style.css` ligne 66

```css
.btn {
  -webkit-appearance: none;
  appearance: none;
  font: inherit;
  font-weight: 600;
  color: var(--text);
  background: var(--panel-alt);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px 14px;
  cursor: pointer;
  transition: background .12s, border-color .12s, color .12s;
…
```

### [STY-03] Barre du haut

Logo, ON AIR, DIFFUSER, OBS, horloge, thèmes, réglages et adaptation aux petites largeurs.

`app/ui/style.css` ligne 92

```css
.topbar {
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 0 16px;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
}
```

### [STY-04] Disposition générale

Grille des platines, du mixeur et de la partie basse.

`app/ui/style.css` ligne 198

```css
.layout {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 12px;
  padding: 12px;
  min-height: 0;
}
```

### [STY-05] Platines

Cadres, effet Rainbow, temps, barre de progression et zone de fondu.

`app/ui/style.css` ligne 215

```css
@property --angle {
  syntax: "<angle>";
  initial-value: 0deg;
  inherits: false;
}
```

### [STY-06] Mixeur et vumètres

Crossfader, fondu, préréglage antenne, vumètres et témoin CLIP.

`app/ui/style.css` ligne 322

```css
.mixer {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
```

### [STY-07] Playlist et historique

Onglets, lignes, titres en attente, marqueurs de dépôt et historique.

`app/ui/style.css` ligne 373

```css
.playlist {
  position: relative;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  display: flex;
  flex-direction: column;
  min-height: 0;
}
```

### [STY-08] Glisser-déposer

Étiquette qui suit le pointeur et curseur pendant le glisser.

`app/ui/style.css` ligne 491

```css
.drag-ghost {
  position: fixed;
  display: none;
  pointer-events: none;
  z-index: 30;
  padding: 6px 12px;
  border-radius: 8px;
  background: var(--accent);
  color: #fff;
  font-weight: 700;
  max-width: 320px;
  white-space: nowrap;
…
```

### [STY-09] Boîtes latérales

Cadre commun de Voice FX et de la Sound FX, onglets et listes de préréglages.

`app/ui/style.css` ligne 510

```css
.fx { background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); padding: 8px 12px; display: flex; flex-direction: column; gap: 6px; }
```

### [STY-10] Voice FX

VERROU, PARLER, ducking, vumètre micro, curseurs et égaliseur.

`app/ui/style.css` ligne 528

```css
.vfx-main { display: grid; grid-template-columns: 78px 1fr 60px; gap: 8px; height: 48px; }
```

### [STY-11] Sound FX

Grille de pads, état de lecture et menu de pad.

`app/ui/style.css` ligne 567

```css
.soundfx { flex: 1 0 196px; min-height: 196px; overflow: hidden; }
```

### [STY-12] Panneau OBS

Boutons matériels, scènes et sources audio.

`app/ui/style.css` ligne 609

```css
.obs-status { display: flex; align-items: center; gap: 8px; font-size: 12px; }
```

### [STY-13] Réglages

Fenêtre des réglages, onglets, formulaires, liste des sorties et onglet À propos.

`app/ui/style.css` ligne 637

```css
.about-head { display: flex; flex-direction: column; gap: 4px; }
```

### [STY-14] Notifications

Pile des notifications et notification de mise à jour.

`app/ui/style.css` ligne 833

```css
.toasts { position: fixed; right: 16px; bottom: 40px; display: flex; flex-direction: column; gap: 8px; z-index: 50; }
```

### [STY-15] Barre d'état

Sortie audio et résumé de la diffusion.

`app/ui/style.css` ligne 852

```css
.statusbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 14px;
  font-size: 12px;
  background: var(--panel);
  border-top: 1px solid var(--border);
}
```

### [STY-16] Masquage

Masque un élément quel que soit son style.

`app/ui/style.css` ligne 887

```css
.hidden { display: none !important; }
```

### [STY-17] Verre dépoli

Image de fond assombrie sous la page ; panneaux translucides qui floutent ce qui est derrière eux (backdrop-filter).

`app/ui/style.css` ligne 863

```css
html.glass body {
  background:
    linear-gradient(rgba(0, 0, 0, var(--glass-dim)), rgba(0, 0, 0, var(--glass-dim))),
    var(--glass-backdrop) center / cover no-repeat fixed;
}
```

### [STY-18] Onglet Serveur Icecast

Mise en page de l'onglet dans les Réglages : liste des serveurs, en-tête, tuiles, graphique, tableaux, formulaires, journal et barre d'enregistrement.

`app/ui/style.css` ligne 742

```css
.tab-page.ice-tab { padding: 0; display: grid; grid-template-columns: 200px minmax(0, 1fr); overflow: hidden; }
```

### [STY-19] Bouton DIFFUSER et son détail

Bouton « + » accolé à DIFFUSER et panneau de détail des sorties.

`app/ui/style.css` ligne 142

```css
.cast-group { display: flex; align-items: stretch; }
```

### [STY-20] Chat

Pastille de messages non lus, état, messages, badges, annonces, champ d'envoi et cartes de demandes.

`app/ui/style.css` ligne 397

```css
.tab-badge { margin-left: 6px; min-width: 16px; padding: 0 5px; border-radius: 8px; background: var(--accent); color: #fff; font-size: 10px; font-weight: 800; letter-spacing: 0; line-height: 16px; display: inline-block; text-align: center; }
```


## HTM

Interface : structure de la page

### [HTM-01] Icônes

Sprite des icônes SVG communes à Linux et Windows.

`app/ui/index.html` ligne 10

```html
<svg class="sprite" aria-hidden="true" style="position:absolute;width:0;height:0;overflow:hidden">
  <symbol id="i-play" viewBox="0 0 24 24"><path d="M7 4.5v15l12.5-7.5z"/></symbol>
  <symbol id="i-pause" viewBox="0 0 24 24"><rect x="5.5" y="4.5" width="4.5" height="15" rx="1.2"/><rect x="14" y="4.5" width="4.5" height="15" rx="1.2"/></symbol>
  <symbol id="i-stop" viewBox="0 0 24 24"><rect x="5.5" y="5.5" width="13" height="13" rx="2"/></symbol>
  <symbol id="i-eject" viewBox="0 0 24 24"><path d="M12 4.5l7.5 9h-15z"/><rect x="4.5" y="16" width="15" height="3.5" rx="1.2"/></symbol>
  <symbol id="i-next" viewBox="0 0 24 24"><path d="M3 5v14l7.5-7zM10.5 5v14l7.5-7z"/><rect x="18.5" y="5" width="2.8" height="14" rx="1"/></symbol>
```

### [HTM-02] Barre du haut

Logo, ON AIR, DIFFUSER, OBS, horloge, thème et réglages.

`app/ui/index.html` ligne 30

```html
<header class="topbar">
  <div class="brand">
    <img src="assets/icon.png" alt="" class="brand-icon">
    <span class="brand-name">BLAST</span>
    <span class="brand-sub">Broadcaster</span>
  </div>
```

### [HTM-03] Platine A

Titre, temps, progression et commandes de la platine A.

`app/ui/index.html` ligne 65

```html
<article class="deck" data-deck="A">
  <div class="deck-head">
    <span class="deck-label">PLATINE A</span>
    <span class="chip state">VIDE</span>
  </div>
  <div class="deck-title">—</div>
```

### [HTM-04] Mixeur

Crossfader, fondu, auto-mix, préréglage antenne et vumètres.

`app/ui/index.html` ligne 92

```html
<section class="mixer">
  <div class="mixer-title">MIX</div>
  <div class="crossfader">
    <span class="xf-label a">A</span>
    <input id="crossfader" type="range" min="0" max="1000" value="0">
    <span class="xf-label b">B</span>
```

### [HTM-05] Platine B

Titre, temps, progression et commandes de la platine B.

`app/ui/index.html` ligne 118

```html
<article class="deck" data-deck="B">
  <div class="deck-head">
    <span class="deck-label">PLATINE B</span>
    <span class="chip state">VIDE</span>
  </div>
  <div class="deck-title">—</div>
```

### [HTM-06] Playlist et historique

Barre d'outils, lignes de playlist et historique de diffusion.

`app/ui/index.html` ligne 147

```html
<section class="playlist" id="playlist">
  <div class="playlist-bar">
    <div class="fx-tabs ptabs">
      <button class="fx-tab active" data-ptab="playlist">PLAYLIST</button>
      <button class="fx-tab" data-ptab="history">HISTORIQUE</button>
      <button class="fx-tab" data-ptab="chat">CHAT<span id="chat-unread" class="tab-badge hidden"></span></button>
```

### [HTM-07] Voice FX

Préréglage de voix, VERROU, PARLER, ducking, curseurs et égaliseur.

`app/ui/index.html` ligne 206

```html
<section class="fx voicefx" id="voicefx">
  <div class="fx-head">
    <span class="fx-title" id="mic-name">VOICE FX</span>
    <label class="fx-select head-select" title="Effet appliqué à la voix"><select id="voice-preset"></select></label>
  </div>
  <div class="vfx-main">
```

### [HTM-08] Sound FX et AURA · OBS

Pads et panneau de pilotage d'OBS.

`app/ui/index.html` ligne 246

```html
<section class="fx soundfx" id="soundfx">
  <div class="fx-tabs">
    <button class="fx-tab active" data-fx="pads">SOUND FX</button>
    <button class="fx-tab" data-fx="jingles">JINGLES</button>
    <button class="fx-tab" data-fx="obs">AURA &middot; OBS</button>
  </div>
```

### [HTM-09] Menu de pad

Renommer, choisir un son, vider.

`app/ui/index.html` ligne 300

```html
<div id="pad-menu" class="pad-menu hidden">
  <input id="pad-name" placeholder="Nom du pad">
  <button class="btn small wide" id="pad-pick">Choisir un son…</button>
  <button class="btn small wide" id="pad-rename">Renommer</button>
  <button class="btn ghost small wide danger" id="pad-clear">Vider le pad</button>
</div>
```

### [HTM-10] Barre d'état

Sortie audio et résumé de diffusion.

`app/ui/index.html` ligne 332

```html
<footer class="statusbar">
  <span id="device" class="muted">Sortie audio…</span>
  <span id="audio-error" class="error-text"></span>
  <span class="spacer"></span>
  <span id="cast-summary" class="muted"></span>
</footer>
```

### [HTM-11] Réglages

Fenêtre des réglages et ses onglets.

`app/ui/index.html` ligne 340

```html
<div id="settings" class="modal hidden" role="dialog" aria-modal="true">
  <div class="modal-panel">
    <aside class="modal-tabs">
      <div class="modal-title">RÉGLAGES</div>
      <button class="tab active" data-tab="cast">Diffusion</button>
      <button class="tab" data-tab="icecast">Serveur Icecast</button>
```

### [HTM-12] Réglages : Diffusion

Liste et formulaire des sorties Icecast / Shoutcast.

`app/ui/index.html` ligne 359

```html
<section class="tab-page" data-page="cast">
  <div class="cast-layout">
    <div class="out-list">
      <div class="out-items" id="out-items"></div>
      <button class="btn small wide" id="out-add">+ Ajouter une sortie</button>
    </div>
```

### [HTM-13] Réglages : Aura / OBS

Connexion à obs-websocket et source « Now Playing ».

`app/ui/index.html` ligne 412

```html
<section class="tab-page hidden" data-page="aura">
  <form class="form" id="aura-form" autocomplete="off">
    <p class="muted">Connexion au serveur WebSocket d'OBS Studio (Outils → Paramètres du serveur WebSocket). Le pilotage se fait dans l'onglet <b>AURA · OBS</b> à côté de la Sound FX. Si une source texte est indiquée ci-dessous, BLAST y écrit « Artiste - Titre » à chaque changement de morceau.</p>
    <div class="grid2">
      <label>Adresse<input name="host" required></label>
      <label>Port<input name="port" type="number" min="1" max="65535" required></label>
```

### [HTM-14] Réglages : Automation

Règles de séparation et conservation de l'historique.

`app/ui/index.html` ligne 462

```html
<section class="tab-page hidden" data-page="automation">
  <form class="form" id="automation-form">
    <p class="muted">Règles de séparation appliquées quand BLAST choisit lui-même le titre suivant (auto-mix, bouton Suivant, FONDU vers une platine vide). Les titres concernés sont marqués d'une horloge dans la playlist et passés en attendant leur tour. Si aucun titre ne respecte les règles, le premier passe quand même et BLAST te prévient.</p>
    <div class="grid2">
      <label class="check span2"><input type="checkbox" name="enabled"> Activer les règles de séparation</label>
      <label>Même artiste : pas avant (minutes)<input type="number" name="artist_minutes" min="0" max="1440" step="5"></label>
```

### [HTM-15] Réglages : Audio

Micro, retour et sortie audio.

`app/ui/index.html` ligne 496

```html
<section class="tab-page hidden" data-page="audio">
  <form class="form" id="mic-form">
    <div class="grid2">
      <label class="span2">Micro (Voice FX)
        <select name="device"></select>
      </label>
```

### [HTM-16] Réglages : À propos

Version, mises à jour et diagnostic.

`app/ui/index.html` ligne 665

```html
<section class="tab-page hidden" data-page="about">
  <div class="form about">
    <div class="about-head">
      <div class="about-name">BLAST Broadcaster</div>
      <div class="muted">Black-Lab Audio Streaming Tool · version <b id="about-version"></b> · <span id="about-platform"></span></div>
    </div>
```

### [HTM-17] Box à jingles

Dossier, relecture, volume, arrêt et grille des jingles.

`app/ui/index.html` ligne 253

```html
<div class="fx-head" data-fxpage="jingles" hidden>
  <button class="btn ghost small" id="jingle-dir" title="Choisir le dossier des jingles">Dossier…</button>
  <button class="btn ghost small pager" id="jingle-rescan" title="Relire le dossier">&#8635;</button>
  <span id="jingle-info" class="muted fx-sub"></span>
  <span class="spacer"></span>
  <input type="range" id="jingle-vol" min="-30" max="6" step="0.5" title="Volume des jingles">
```

### [HTM-18] Filtres de l'historique

Tout, Morceaux, Jingles, Pubs.

`app/ui/index.html` ligne 158

```html
<div class="hfilters" data-ppage="history" hidden>
  <button class="chip-btn active" data-hfilter="all">Tout</button>
  <button class="chip-btn" data-hfilter="morceau">Morceaux</button>
  <button class="chip-btn" data-hfilter="jingle">Jingles</button>
  <button class="chip-btn" data-hfilter="pub">Pubs</button>
</div>
```

### [HTM-19] Menu d'export

Catégorie et période de l'export CSV de l'historique.

`app/ui/index.html` ligne 308

```html
<div id="export-menu" class="pad-menu export-menu hidden">
  <div class="menu-title">Exporter l'historique (CSV)</div>
  <label>Catégorie
    <select id="export-kind">
      <option value="all">Tout</option>
      <option value="morceau">Morceaux</option>
```

### [HTM-20] Réglages : Catégories

Mots-clés de dossier des jingles et des pubs.

`app/ui/index.html` ligne 481

```html
<form class="form" id="categories-form">
  <p class="muted">Un fichier rangé dans un dossier dont le nom contient l'un de ces mots (au singulier ou au pluriel, majuscules et accents ignorés) est classé jingle ou pub ; tout le reste est un morceau. Jingles et pubs ne sont pas soumis aux règles de séparation et ne changent pas le titre annoncé aux auditeurs. Plusieurs mots : séparés par des virgules.</p>
  <div class="grid2">
    <label>Dossiers de jingles<input name="jingle_keywords" placeholder="JINGLE"></label>
    <label>Dossiers de pubs<input name="pub_keywords" placeholder="PUB, PUBLICITE"></label>
  </div>
```

### [HTM-21] Réglages : Apparence

Verre dépoli, image de fond et curseurs de flou, d'opacité et d'assombrissement.

`app/ui/index.html` ligne 640

```html
<section class="tab-page hidden" data-page="appearance">
  <form class="form" id="appearance-form">
    <p class="muted">Verre dépoli : BLAST s'affiche sur une image de fond de ton choix, et ses panneaux deviennent translucides et floutent l'image derrière eux. L'image est copiée dans le dossier de BLAST. Les réglages s'appliquent en direct ; le flou demande un peu de carte graphique : décoche si l'interface ralentit.</p>
    <label class="check"><input type="checkbox" name="glass"> Activer l'effet verre dépoli</label>
    <div class="bg-pick">
      <div class="bg-thumb" id="bg-thumb"></div>
```

### [HTM-22] Réglages : Grilles

Bibliothèque, catégories, grilles et planning de la semaine.

`app/ui/index.html` ligne 529

```html
<section class="tab-page hidden" data-page="rotation">
  <div class="hfilters rot-nav">
    <button class="chip-btn active" data-rot="library">Bibliothèque</button>
    <button class="chip-btn" data-rot="categories">Catégories</button>
    <button class="chip-btn" data-rot="clocks">Grilles</button>
    <button class="chip-btn" data-rot="programs">Émissions</button>
```

### [HTM-23] Menu Mélanger

Actions de mélange et de nettoyage de la playlist.

`app/ui/index.html` ligne 325

```html
<div id="shuffle-menu" class="pad-menu shuffle-menu hidden">
  <button class="btn ghost small" data-shuffle="plain">Mélanger</button>
  <button class="btn ghost small" data-shuffle="spread">Mélanger en espaçant les artistes</button>
  <button class="btn ghost small" data-shuffle="dedupe">Retirer les doublons</button>
</div>
```

### [HTM-24] Page Émissions

Émissions des grilles : nom, couleur, fichier M3U, explications.

`app/ui/index.html` ligne 608

```html
<div class="rot-page rot-split" data-rotpage="programs" hidden>
  <div class="rot-side">
    <div id="prog-list" class="rot-list"></div>
    <button type="button" class="btn small" id="prog-add">Nouvelle émission</button>
  </div>
  <form class="form rot-form" id="prog-form">
```

### [HTM-25] Réglages repliables du Voice FX

Égaliseur micro et temps du ducking ; repliés sur une même ligne, une section ouverte prend toute la largeur.

`app/ui/index.html` ligne 225

```html
<div class="vfx-more">
<details class="eq">
  <summary>Égaliseur micro</summary>
  <div class="sliders">
    <label><span>Grave</span><input type="range" id="eq-low" min="-12" max="12" step="0.5"><output id="eq-low-v"></output></label>
    <label><span>Médium</span><input type="range" id="eq-mid" min="-12" max="12" step="0.5"><output id="eq-mid-v"></output></label>
```

### [HTM-26] Onglet Serveur Icecast

Onglet des Réglages : liste des serveurs à gauche, contenu du serveur choisi à droite.

`app/ui/index.html` ligne 691

```html
<section class="tab-page hidden ice-tab" data-page="icecast">
  <div class="ice-side">
    <div class="form-section">Serveurs</div>
    <div class="ice-servers" id="ice-servers"></div>
    <button class="btn small wide" id="ice-add">+ Ajouter un serveur</button>
  </div>
```

### [HTM-27] Accueil Icecast

Présentation, installations détectées et ajout d'un serveur local ou distant.

`app/ui/index.html` ligne 699

```html
<div class="ice-welcome" id="ice-welcome">
  <div class="about-name">Serveur Icecast</div>
  <p class="muted">Lance, configure et surveille ton serveur Icecast sans éditer <code>icecast.xml</code> à la main. Un serveur <b>local</b> est piloté entièrement (démarrage, configuration, journaux). Un serveur <b>distant</b> (VPS) est surveillé et administré par son interface admin.</p>
  <div class="form-section">Installations trouvées sur cet ordinateur</div>
  <div class="ice-detected" id="ice-detected"></div>
  <div class="form-actions">
```

### [HTM-28] En-tête du serveur

État, nom, adresse, version, boutons page web, Redémarrer, Arrêter et Démarrer.

`app/ui/index.html` ligne 712

```html
<header class="ice-head">
  <span class="dot" id="ice-dot"></span>
  <div class="ice-ident">
    <div class="ice-name" id="ice-name"></div>
    <div class="ice-sub muted" id="ice-sub"></div>
  </div>
```

### [HTM-29] Tableau de bord Icecast

Tuiles (auditeurs, pic, sources, durée, débit, connexions), graphique des auditeurs et tableau des mountpoints ; sans flux, explication du point de montage et bouton de sortie vers /live.

`app/ui/index.html` ligne 734

```html
<div class="ice-page" data-ipage="dash">
  <div class="ice-banner hidden" id="ice-banner"></div>
  <div class="ice-tiles">
    <div class="ice-tile"><small>Auditeurs</small><b id="it-listeners">—</b><span id="it-listeners-sub" class="muted"></span></div>
    <div class="ice-tile"><small>Pic</small><b id="it-peak">—</b><span class="muted">record des mountpoints actifs</span></div>
    <div class="ice-tile"><small>Sources</small><b id="it-sources">—</b><span id="it-sources-sub" class="muted"></span></div>
```

### [HTM-30] Configuration Icecast

Fichier, droit d'écriture et formulaire : serveur, sécurité, ports d'écoute, limites, chemins, journalisation.

`app/ui/index.html` ligne 761

```html
<div class="ice-page hidden" data-ipage="config">
  <div class="ice-file" id="ice-file">
    <code id="ice-file-path"></code>
    <span class="chip" id="ice-file-state"></span>
    <span class="muted" id="ice-file-backups"></span>
    <span class="spacer"></span>
```

### [HTM-31] Mountpoints Icecast

Explication (onglet facultatif), liste des mountpoints déclarés et formulaire : accès, secours, présentation, annuaire, sortie BLAST.

`app/ui/index.html` ligne 826

```html
<div class="ice-page hidden" data-ipage="mounts">
  <p class="muted ice-intro">Facultatif. Icecast accepte n'importe quel point de montage avec le mot de passe des sources : déclare-en un ici seulement pour lui donner des règles (limite d'auditeurs, mot de passe propre, flux de secours si ta diffusion tombe, nom et genre imposés).</p>
  <div class="cast-layout">
    <div class="out-list">
      <div class="out-items" id="ice-mount-list"></div>
      <button class="btn small wide" id="ice-mount-add">+ Nouveau mountpoint</button>
```

### [HTM-32] Auditeurs Icecast

Choix du mountpoint et tableau des auditeurs avec déconnexion.

`app/ui/index.html` ligne 877

```html
<div class="ice-page hidden" data-ipage="listeners">
  <div class="form-actions ice-tools">
    <label class="fx-select">Mountpoint <select id="ice-lis-mount"></select></label>
    <span class="muted" id="ice-lis-info"></span>
    <span class="spacer"></span>
    <button class="btn small" id="ice-lis-refresh">Actualiser</button>
```

### [HTM-33] Journal Icecast

Erreurs, accès ou console, suivi en direct.

`app/ui/index.html` ligne 894

```html
<div class="ice-page hidden" data-ipage="log">
  <div class="form-actions ice-tools">
    <button class="chip-btn active" data-ilog="error">Erreurs</button>
    <button class="chip-btn" data-ilog="access">Accès</button>
    <button class="chip-btn" data-ilog="console">Console</button>
    <span class="spacer"></span>
```

### [HTM-34] Connexion Icecast

Profil : emplacement, installation, configuration, accès admin et démarrage.

`app/ui/index.html` ligne 907

```html
<div class="ice-page hidden" data-ipage="profile">
  <form class="form" id="ice-profile-form" autocomplete="off">
    <div class="grid2">
      <label>Nom<input name="name" required></label>
      <label>Emplacement
        <select name="remote"><option value="false">Local (cet ordinateur)</option><option value="true">Distant (VPS, serveur dédié)</option></select>
```

### [HTM-35] Barre d'enregistrement Icecast

Apparaît quand la configuration a changé : annuler, enregistrer, enregistrer et redémarrer.

`app/ui/index.html` ligne 945

```html
<footer class="ice-savebar hidden" id="ice-savebar">
  <span>Modifications de la configuration non enregistrées</span>
  <span class="form-result" id="ice-save-result"></span>
  <span class="spacer"></span>
  <button class="btn ghost small" id="ice-revert">Annuler</button>
  <button class="btn small" id="ice-save">Enregistrer</button>
```

### [HTM-36] Menu d'actions Icecast

Menu flottant des actions sur un mountpoint.

`app/ui/index.html` ligne 973

```html
<div id="ice-menu" class="pad-menu ice-menu hidden"></div>

<div id="toasts" class="toasts"></div>
<div id="ghost" class="drag-ghost"></div>

<script type="module" src="js/main.js"></script>
```

### [HTM-37] Panneau de diffusion

Panneau flottant sous le bouton DIFFUSER : en-tête, titre, sorties, total et boutons.

`app/ui/index.html` ligne 960

```html
<div id="cast-panel" class="pad-menu cast-panel hidden">
  <div class="cast-panel-head"><b id="cp-state"></b><span class="spacer"></span><span class="muted" id="cp-time"></span></div>
  <div class="muted cp-song" id="cp-song"></div>
  <div class="cp-outputs" id="cp-outputs"></div>
  <div class="cp-total muted" id="cp-total"></div>
  <div class="form-actions">
```

### [HTM-38] Onglet CHAT

Messages, champ d'envoi et file des demandes, à côté de la playlist et de l'historique.

`app/ui/index.html` ligne 184

```html
<div class="chat-page" data-ppage="chat" hidden>
  <div class="chat-main">
    <div class="chat-log" id="chat-log"></div>
    <form class="chat-form" id="chat-form" autocomplete="off">
      <input id="chat-input" maxlength="480" placeholder="Écrire dans le chat…">
      <button type="submit" class="btn small accent" id="chat-send">Envoyer</button>
```

### [HTM-39] Réglages du chat

Chaîne, connexion au démarrage, compte et jeton, commandes des viewers.

`app/ui/index.html` ligne 431

```html
<section class="tab-page hidden" data-page="chat">
  <form class="form" id="chat-settings-form" autocomplete="off">
    <p class="muted">Chat Twitch dans l'onglet <b>CHAT</b> à côté de la playlist. La lecture est anonyme : seule la chaîne est nécessaire. Pour que BLAST réponde aux commandes et écrive dans le chat, ajoute un compte (ta chaîne ou un compte bot) et son jeton.</p>
    <div class="grid2">
      <label>Chaîne Twitch<input name="channel" placeholder="nom de la chaîne ou lien twitch.tv/…"></label>
      <label class="check"><input name="autoconnect" type="checkbox"> Se connecter au chat au démarrage de BLAST</label>
```
