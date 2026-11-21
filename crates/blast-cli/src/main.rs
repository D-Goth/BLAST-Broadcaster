use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use blast_core::engine::{self, Engine};
use blast_core::{decode_file, Command, DeckId, DeckState, DeckStatus, Event, Mixer, Track};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "blast",
    version,
    about = "BLAST Broadcaster — outil de test du moteur audio (M1)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

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
    Play {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(
            help = "Durée du fondu enchaîné, en secondes",
            short,
            long,
            default_value_t = 6.0
        )]
        fade: f32,
        #[arg(
            help = "Nom exact de la sortie audio (voir `blast devices`)",
            short,
            long
        )]
        device: Option<String>,
    },
    #[command(about = "Calcule le mix hors-ligne et l'enregistre dans un fichier WAV")]
    Render {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long, default_value_t = 6.0)]
        fade: f32,
        #[arg(long, default_value_t = 48_000)]
        rate: u32,
        #[arg(help = "WAV 16 bits au lieu de 32 bits flottants", long)]
        pcm16: bool,
    },
}

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
            output,
            fade,
            rate,
            pcm16,
        } => render(&files, &output, fade, rate, pcm16),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur : {e}");
            ExitCode::FAILURE
        }
    }
}

type AnyResult = Result<(), Box<dyn std::error::Error>>;

fn clock(frames: usize, rate: u32) -> String {
    let secs = frames as u64 / rate.max(1) as u64;
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

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
    }
    Ok(())
}

struct Sequencer {
    files: Vec<PathBuf>,
    next: usize,
    finished: usize,
    rate: u32,
    names: [String; 2],
}

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
        while self.next < self.files.len() {
            let path = self.files[self.next].clone();
            self.next += 1;
            match decode_file(&path, self.rate) {
                Ok(t) => {
                    self.names[deck.index()] = t.display_name();
                    return Some(Arc::new(t));
                }
                Err(e) => {
                    eprintln!("\nignoré : {} ({e})", path.display());
                    self.finished += 1;
                }
            }
        }
        self.names[deck.index()].clear();
        None
    }

    fn done(&self) -> bool {
        self.finished >= self.files.len()
    }

    fn start(&mut self, mut send: impl FnMut(Command), fade: f32) {
        send(Command::SetAutoMix {
            enabled: true,
            fade_frames: (fade.max(0.0) * self.rate as f32) as usize,
        });
        send(Command::SetCrossfader(0.0));
        if let Some(t) = self.load_next(DeckId::A) {
            send(Command::Load(DeckId::A, t));
            send(Command::Play(DeckId::A));
        }
        if let Some(t) = self.load_next(DeckId::B) {
            send(Command::Load(DeckId::B, t));
        }
    }

    fn on_ended(&mut self, deck: DeckId, mut send: impl FnMut(Command)) {
        self.finished += 1;
        if let Some(t) = self.load_next(deck) {
            send(Command::Load(deck, t));
        } else {
            send(Command::Eject(deck));
        }
    }
}

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
        clock(s.frames, rate),
        short
    )
}

fn fader(pos: f32) -> String {
    let width = 11;
    let cursor = (pos.clamp(0.0, 1.0) * (width - 1) as f32).round() as usize;
    (0..width)
        .map(|i| if i == cursor { '●' } else { '━' })
        .collect()
}

fn meter(peak: f32) -> String {
    let width = 10;
    let filled = (peak.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("{}{}", "█".repeat(filled), " ".repeat(width - filled))
}

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
    seq.start(
        |c| {
            engine.send(c);
        },
        fade,
    );

    while !seq.done() {
        let Some(event) = engine.next_event(Duration::from_millis(250)) else {
            continue;
        };
        match event {
            Event::Tick {
                decks,
                crossfader,
                peak,
            } => {
                print!(
                    "\r{}  {}  {}  {}",
                    deck_line(DeckId::A, &decks[0], rate, &seq.names[0]),
                    fader(crossfader),
                    deck_line(DeckId::B, &decks[1], rate, &seq.names[1]),
                    meter(peak[0].max(peak[1]))
                );
                let _ = std::io::stdout().flush();
            }
            Event::AutoTransition { from, to } => {
                println!("\n↪ fondu {from} → {to} : {}", seq.names[to.index()]);
            }
            Event::Ended(deck) => {
                seq.on_ended(deck, |c| {
                    engine.send(c);
                });
                engine.collect_garbage();
            }
            Event::CrossfadeDone { .. } => {}
        }
    }
    println!("\nFin de la liste.");
    Ok(())
}

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
        bits_per_sample: if pcm16 { 16 } else { 32 },
        sample_format: if pcm16 {
            hound::SampleFormat::Int
        } else {
            hound::SampleFormat::Float
        },
    };
    let mut writer = hound::WavWriter::create(output, spec)?;
    let mut block = vec![0.0f32; 1024 * 2];
    let mut events: Vec<Event> = Vec::new();
    let mut written = 0usize;

    while !seq.done() {
        mixer.render(&mut block, |e| {
            if !matches!(e, Event::Tick { .. }) {
                events.push(e)
            }
        });
        for &s in &block {
            if pcm16 {
                writer.write_sample((s * i16::MAX as f32) as i16)?;
            } else {
                writer.write_sample(s)?;
            }
        }
        written += block.len() / 2;
        for e in events.drain(..) {
            match e {
                Event::AutoTransition { from, to } => println!(
                    "{}  fondu {from} → {to} : {}",
                    clock(written, rate),
                    seq.names[to.index()]
                ),
                Event::Ended(deck) => {
                    seq.on_ended(deck, |c| pending.push(c));
                }
                _ => {}
            }
        }
        for c in pending.drain(..) {
            mixer.apply(c);
        }
    }
    writer.finalize()?;
    println!(
        "{}  écrit : {} ({} Hz, {})",
        clock(written, rate),
        output.display(),
        rate,
        if pcm16 {
            "16 bits"
        } else {
            "32 bits flottants"
        }
    );
    Ok(())
}
