//! Standalone microphone/speaker diagnostic.
//!
//! This intentionally bypasses VoiceInput. It plays a supplied WAV through
//! the Windows default render endpoint while capturing the default input
//! endpoint with cpal, then reports whether acoustic energy reached the mic.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const CAPTURE_SECONDS: u64 = 12;

fn write_wav(path: &Path, samples: &[i16], sample_rate: u32) -> io::Result<()> {
    let mut file = File::create(path)?;
    let data_bytes = (samples.len() * 2) as u32;
    let riff_size = 36 + data_bytes;
    file.write_all(b"RIFF")?;
    file.write_all(&riff_size.to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&(sample_rate * 2).to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_bytes.to_le_bytes())?;
    for sample in samples {
        file.write_all(&sample.to_le_bytes())?;
    }
    Ok(())
}

fn print_metrics(samples: &[i16]) {
    let mut sum_sq = 0.0f64;
    let mut peak = 0.0f32;
    let mut above_vad = 0usize;
    let mut above_any = 0usize;
    for &sample in samples {
        let value = (sample as f32 / 32768.0).abs();
        peak = peak.max(value);
        sum_sq += f64::from(value * value);
        if value > 10f32.powf(-40.0 / 20.0) {
            above_vad += 1;
        }
        if value > 0.001 {
            above_any += 1;
        }
    }
    let rms = if samples.is_empty() {
        0.0
    } else {
        (sum_sq / samples.len() as f64).sqrt()
    };
    println!("captured_samples={}", samples.len());
    println!("rms={rms:.6} rms_db={:.1}", 20.0 * rms.max(1e-12).log10());
    println!("peak={peak:.6} peak_db={:.1}", 20.0 * peak.max(1e-12).log10());
    println!(
        "samples_above_-40db={} ({:.2}%)",
        above_vad,
        if samples.is_empty() {
            0.0
        } else {
            above_vad as f64 * 100.0 / samples.len() as f64
        }
    );
    println!(
        "samples_above_-60db={} ({:.2}%)",
        above_any,
        if samples.is_empty() {
            0.0
        } else {
            above_any as f64 * 100.0 / samples.len() as f64
        }
    );
}

fn playback(path: &str) -> io::Result<()> {
    let command = format!(
        "$p=New-Object System.Media.SoundPlayer '{}'; $p.PlaySync(); $p.Dispose()",
        path.replace('\'', "''")
    );
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &command])
        .spawn()?;
    let _ = child.wait();
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wav_path = env::args()
        .nth(1)
        .ok_or("usage: audio_hardware_probe.exe <wav-path> [output-wav]")?;
    let output_path = env::args()
        .nth(2)
        .unwrap_or_else(|| "test-artifacts/audio_probe_mic.wav".to_string());
    let should_play = env::args().nth(3).as_deref() != Some("--no-playback");

    let host = cpal::default_host();
    let input = host
        .default_input_device()
        .ok_or("no default input device")?;
    let output = host
        .default_output_device()
        .ok_or("no default output device")?;
    let input_name = input.name().unwrap_or_else(|_| "<unknown>".to_string());
    let output_name = output.name().unwrap_or_else(|_| "<unknown>".to_string());
    let supported = input.default_input_config()?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels();
    println!("default_output={output_name}");
    println!("default_input={input_name}");
    println!(
        "input_config={}Hz {}ch {:?}",
        sample_rate,
        channels,
        supported.sample_format()
    );
    println!("playback_wav={wav_path}");
    println!("playback_enabled={should_play}");
    println!("capture_seconds={CAPTURE_SECONDS}");

    let samples = Arc::new(Mutex::new(Vec::<i16>::new()));
    let samples_for_callback = samples.clone();
    let stream_config: StreamConfig = supported.clone().into();
    let error_callback = |error| eprintln!("input_stream_error={error}");
    let stream = match supported.sample_format() {
        SampleFormat::I16 => input.build_input_stream(
            &stream_config,
            move |data: &[i16], _| {
                let mut buffer = samples_for_callback.lock().unwrap_or_else(|e| e.into_inner());
                if channels == 1 {
                    buffer.extend_from_slice(data);
                } else {
                    for frame in data.chunks(channels as usize) {
                        let average = frame.iter().map(|&x| x as i64).sum::<i64>()
                            / frame.len() as i64;
                        buffer.push(average as i16);
                    }
                }
            },
            error_callback,
            None,
        )?,
        SampleFormat::F32 => input.build_input_stream(
            &stream_config,
            move |data: &[f32], _| {
                let mut buffer = samples_for_callback.lock().unwrap_or_else(|e| e.into_inner());
                for frame in data.chunks(channels as usize) {
                    let average = frame.iter().copied().sum::<f32>() / frame.len() as f32;
                    buffer.push((average.clamp(-1.0, 1.0) * 32767.0) as i16);
                }
            },
            error_callback,
            None,
        )?,
        SampleFormat::U16 => input.build_input_stream(
            &stream_config,
            move |data: &[u16], _| {
                let mut buffer = samples_for_callback.lock().unwrap_or_else(|e| e.into_inner());
                for frame in data.chunks(channels as usize) {
                    let average = frame.iter().map(|&x| x as i64 - 32768).sum::<i64>()
                        / frame.len() as i64;
                    buffer.push(average as i16);
                }
            },
            error_callback,
            None,
        )?,
        format => return Err(format!("unsupported input sample format: {format:?}").into()),
    };

    stream.play()?;
    thread::sleep(Duration::from_millis(500));
    if should_play {
        let _ = playback(&wav_path);
    }
    thread::sleep(Duration::from_secs(CAPTURE_SECONDS));
    drop(stream);

    let samples = samples.lock().unwrap_or_else(|e| e.into_inner()).clone();
    write_wav(Path::new(&output_path), &samples, sample_rate)?;
    println!("capture_wav={output_path}");
    print_metrics(&samples);
    Ok(())
}
