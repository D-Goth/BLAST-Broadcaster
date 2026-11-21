use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use crossbeam_channel::{bounded, select, Receiver, Sender};

use crate::{Command, Event, Mixer, Track};

const SCRATCH_FRAMES: usize = 4096;
const TAP_SECONDS: usize = 2;

#[derive(Debug)]
pub enum EngineError {
    NoDevice,
    DeviceNotFound(String),
    UnsupportedFormat(String),
    Backend(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::NoDevice => write!(f, "aucune sortie audio disponible"),
            EngineError::DeviceNotFound(n) => write!(f, "sortie audio introuvable : {n}"),
            EngineError::UnsupportedFormat(s) => {
                write!(f, "format de sortie non pris en charge : {s}")
            }
            EngineError::Backend(e) => write!(f, "erreur audio : {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

fn backend<E: std::fmt::Display>(e: E) -> EngineError {
    EngineError::Backend(e.to_string())
}

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

pub fn default_output_device() -> Option<String> {
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicInput {
    Off,
    Default,
    Named(String),
}

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

pub fn default_input_device() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.name().ok())
}

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
    device_name: String,
}

impl Engine {
    pub fn start(device: Option<&str>) -> Result<Self, EngineError> {
        Self::start_with(device, MicInput::Off)
    }

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
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels;

        let (cmd_tx, cmd_rx) = bounded::<Command>(256);
        let (evt_tx, evt_rx) = bounded::<Event>(1024);
        let (tick_tx, tick_rx) = bounded::<Event>(16);
        let (trash_tx, trash_rx) = bounded::<Arc<Track>>(64);
        let mut mixer = Mixer::new(sample_rate);
        let (input, input_name, input_error) = match open_mic(&host, &mic, sample_rate) {
            Ok(Some((stream, name, consumer))) => {
                mixer.attach_mic(consumer);
                (Some(stream), Some(name), None)
            }
            Ok(None) => (None, None, None),
            Err(e) => (None, None, Some(e.to_string())),
        };
        let (tap_tx, tap_rx) = rtrb::RingBuffer::<f32>::new(sample_rate as usize * 2 * TAP_SECONDS);
        let parts = Parts {
            tap: tap_tx,
            mixer,
            commands: cmd_rx,
            events: evt_tx,
            ticks: tick_tx,
            trash: trash_tx,
        };

        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, parts),
            SampleFormat::I16 => build::<i16>(&device, &config, parts),
            SampleFormat::U16 => build::<u16>(&device, &config, parts),
            SampleFormat::I32 => build::<i32>(&device, &config, parts),
            other => return Err(EngineError::UnsupportedFormat(format!("{other:?}"))),
        }?;
        stream.play().map_err(backend)?;

        if let Some(i) = &input {
            let _ = i.play();
        }

        Ok(Self {
            _stream: stream,
            _input: input,
            input_name,
            input_error,
            commands: cmd_tx,
            events: evt_rx,
            ticks: tick_rx,
            trash: trash_rx,
            tap: tap_rx,
            sample_rate,
            channels,
            device_name,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn input_name(&self) -> Option<&str> {
        self.input_name.as_deref()
    }

    pub fn input_error(&self) -> Option<&str> {
        self.input_error.as_deref()
    }

    pub fn send(&self, cmd: Command) -> bool {
        self.commands.send(cmd).is_ok()
    }

    fn latest_tick(&self, first: Event) -> Event {
        let mut last = first;
        while let Ok(t) = self.ticks.try_recv() {
            last = t;
        }
        last
    }

    pub fn try_event(&self) -> Option<Event> {
        if let Ok(e) = self.events.try_recv() {
            return Some(e);
        }
        self.ticks.try_recv().ok().map(|t| self.latest_tick(t))
    }

    pub fn next_event(&self, timeout: Duration) -> Option<Event> {
        if let Ok(e) = self.events.try_recv() {
            return Some(e);
        }
        select! {
            recv(self.events) -> e => e.ok(),
            recv(self.ticks) -> t => t.ok().map(|t| self.latest_tick(t)),
            default(timeout) => None,
        }
    }

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
    }

    pub fn collect_garbage(&self) {
        while self.trash.try_recv().is_ok() {}
    }
}

struct Parts {
    tap: rtrb::Producer<f32>,
    mixer: Mixer,
    commands: Receiver<Command>,
    events: Sender<Event>,
    ticks: Sender<Event>,
    trash: Sender<Arc<Track>>,
}

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
        commands,
        events,
        ticks,
        trash,
    } = parts;
    let mut scratch = vec![0.0f32; SCRATCH_FRAMES * 2];
    let mut local_scratch = vec![0.0f32; SCRATCH_FRAMES * 2];

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                while let Ok(cmd) = commands.try_recv() {
                    if let Some(old) = mixer.apply(cmd) {
                        let _ = trash.try_send(old);
                    }
                }
                while let Some(old) = mixer.take_retired() {
                    if trash.try_send(old).is_err() {
                        break;
                    }
                }
                let total = data.len() / channels.max(1);
                let mut done = 0;
                while done < total {
                    let n = (total - done).min(SCRATCH_FRAMES);
                    let buf = &mut scratch[..n * 2];
                    let local = &mut local_scratch[..n * 2];
                    mixer.render_split(Some(&mut *local), buf, |e| {
                        let _ = match e {
                            Event::Tick { .. } => ticks.try_send(e),
                            _ => events.try_send(e),
                        };
                    });
                    let room = tap.slots().min(buf.len()) & !1;
                    if room > 0 {
                        if let Ok(chunk) = tap.write_chunk_uninit(room) {
                            chunk.fill_from_iter(buf[..room].iter().copied());
                        }
                    }
                    for f in 0..n {
                        let frame = &mut data[(done + f) * channels..(done + f + 1) * channels];
                        let l = local[2 * f];
                        let r = local[2 * f + 1];
                        match channels {
                            1 => frame[0] = T::from_sample((l + r) * 0.5),
                            _ => {
                                frame[0] = T::from_sample(l);
                                frame[1] = T::from_sample(r);
                                for s in frame.iter_mut().skip(2) {
                                    *s = T::from_sample(0.0f32);
                                }
                            }
                        }
                    }
                    done += n;
                }
            },
            |err| eprintln!("flux audio : {err}"),
            None,
        )
        .map_err(backend)
}

type MicStream = (cpal::Stream, String, rtrb::Consumer<f32>);

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
            .input_devices()
            .map_err(backend)?
            .find(|d| d.name().map(|n| &n == name).unwrap_or(false))
            .ok_or_else(|| EngineError::DeviceNotFound(name.clone()))?,
    };
    let name = device.name().unwrap_or_default();
    let supported = device.default_input_config().map_err(backend)?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let (tx, rx) = rtrb::RingBuffer::<f32>::new(out_rate as usize);
    let stream = match format {
        SampleFormat::F32 => build_input::<f32>(&device, &config, tx, out_rate),
        SampleFormat::I16 => build_input::<i16>(&device, &config, tx, out_rate),
        SampleFormat::U16 => build_input::<u16>(&device, &config, tx, out_rate),
        SampleFormat::I32 => build_input::<i32>(&device, &config, tx, out_rate),
        other => return Err(EngineError::UnsupportedFormat(format!("micro {other:?}"))),
    }?;
    Ok(Some((stream, name, rx)))
}

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
    let mut phase = 0.0f64;
    let mut prev = 0.0f32;
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                for frame in data.chunks(channels) {
                    let mut sum = 0.0f32;
                    for s in frame {
                        sum += s.to_sample::<f32>();
                    }
                    let cur = sum / frame.len() as f32;
                    while phase < 1.0 {
                        let v = prev + (cur - prev) * phase as f32;
                        if tx.push(v).is_err() {
                            break;
                        }
                        phase += step;
                    }
                    phase -= 1.0;
                    prev = cur;
                }
            },
            |err| eprintln!("micro : {err}"),
            None,
        )
        .map_err(backend)
}
