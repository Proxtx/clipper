use {
  songbird::input::{Input, RawAdapter},
  std::{f32::consts::TAU, io::Cursor, path::PathBuf},
};

/// The sound played in the voice channel when a trigger word is heard.
/// Either the file from TRIGGER_SOUND (wav, mp3, ...) or a built-in chime.
pub enum Sound {
  File(Vec<u8>),
  Chime(Vec<u8>),
}

impl Sound {
  pub fn from_env() -> Sound {
    if let Ok(path) = std::env::var("TRIGGER_SOUND") {
      match std::fs::read(PathBuf::from(&path)) {
        Ok(data) => return Sound::File(data),
        Err(e) => println!(
          "Was unable to read TRIGGER_SOUND {}: {}, using chime",
          path, e
        ),
      }
    }
    Sound::Chime(chime())
  }

  pub fn input(&self) -> Input {
    match self {
      Sound::File(data) => Input::from(data.clone()),
      Sound::Chime(pcm) => RawAdapter::new(Cursor::new(pcm.clone()), 48_000, 2).into(),
    }
  }
}

/// Two rising tones as interleaved stereo f32 little endian PCM at 48kHz.
fn chime() -> Vec<u8> {
  const RATE: f32 = 48_000.0;
  const TONE_SAMPLES: usize = 48_000 * 15 / 100;

  let mut pcm = Vec::with_capacity(TONE_SAMPLES * 2 * 2 * 4);
  for frequency in [880.0, 1320.0] {
    for i in 0..TONE_SAMPLES {
      let t = i as f32 / RATE;
      // short fade in and exponential decay to avoid clicks
      let envelope = (i as f32 / 240.0).min(1.0) * (-t * 12.0).exp();
      let sample = (TAU * frequency * t).sin() * envelope * 0.4;
      for _ in 0..2 {
        pcm.extend_from_slice(&sample.to_le_bytes());
      }
    }
  }
  pcm
}
