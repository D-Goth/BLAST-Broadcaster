use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use crossbeam_channel::{bounded, select, Receiver, Sender};

use crate::{Command, Event, Mixer, Track};

const SCRATCH_FRAMES: usize = 4096;

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

pub struct Engine {
    _stream: cpal::Stream,
    commands: Sender<Command>,
    events: Receiver<Event>,
    ticks: Receiver<Event>,
    trash: Receiver<Arc<Track>>,
    sample_rate: u32,
    channels: u16,
    device_name: String,
}

impl Engine {
    pub fn start(device: Option<&str>) -> Result<Self, EngineError> {
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
        let mixer = Mixer::new(sample_rate);
        let parts = Parts {
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

        Ok(Self {
            _stream: stream,
            commands: cmd_tx,
            events: evt_rx,
            ticks: tick_rx,
            trash: trash_rx,
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

    pub fn collect_garbage(&self) {
        while self.trash.try_recv().is_ok() {}
    }
}

struct Parts {
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
        mut mixer,
        commands,
        events,
        ticks,
        trash,
    } = parts;
    let mut scratch = vec![0.0f32; SCRATCH_FRAMES * 2];

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                while let Ok(cmd) = commands.try_recv() {
                    if let Some(old) = mixer.apply(cmd) {
                        let _ = trash.try_send(old);
                    }
                }
                let total = data.len() / channels.max(1);
                let mut done = 0;
                while done < total {
                    let n = (total - done).min(SCRATCH_FRAMES);
                    let buf = &mut scratch[..n * 2];
                    mixer.render(buf, |e| {
                        let _ = match e {
                            Event::Tick { .. } => ticks.try_send(e),
                            _ => events.try_send(e),
                        };
                    });
                    for f in 0..n {
                        let frame = &mut data[(done + f) * channels..(done + f + 1) * channels];
                        let l = buf[2 * f];
                        let r = buf[2 * f + 1];
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
