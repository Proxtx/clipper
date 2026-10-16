use {
  serenity::model::prelude::GuildId,
  std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
  },
};

pub type DirectorImplementation = Arc<Mutex<Director>>;

pub const SAMPLING_RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
/// songbird delivers one voice tick every 20ms
pub const FRAME_DURATION: Duration = Duration::from_millis(20);
/// interleaved stereo samples per 20ms frame
pub const FRAME_SAMPLES: usize =
  (SAMPLING_RATE as usize / 1000) * FRAME_DURATION.as_millis() as usize * CHANNELS as usize;

/// Rolling buffer of mixed 20ms frames for one guild.
pub struct Composer {
  frames: VecDeque<Vec<i16>>,
  max_frames: usize,
}

impl Composer {
  pub fn new(clip_duration: Duration) -> Self {
    let max_frames = (clip_duration.as_millis() / FRAME_DURATION.as_millis()).max(1) as usize;
    Composer {
      frames: VecDeque::with_capacity(max_frames),
      max_frames,
    }
  }

  /// Mixes all speakers of one tick into a single frame and appends it.
  pub fn add_tick<'a>(&mut self, voices: impl Iterator<Item = &'a [i16]>) {
    let mut mixed = vec![0i32; FRAME_SAMPLES];

    for voice in voices {
      for (sample, value) in mixed.iter_mut().zip(voice) {
        *sample += *value as i32;
      }
    }

    self.frames.push_back(
      mixed
        .into_iter()
        .map(|v| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16)
        .collect(),
    );

    while self.frames.len() > self.max_frames {
      self.frames.pop_front();
    }
  }

  pub fn duration(&self) -> Duration {
    FRAME_DURATION * self.frames.len() as u32
  }

  pub fn compose(&self) -> Vec<i16> {
    self.frames.iter().flatten().copied().collect()
  }
}

pub struct Director {
  composers: HashMap<GuildId, Composer>,
  clip_duration: Duration,
}

impl Director {
  pub fn new(clip_duration: Option<Duration>) -> Self {
    Director {
      composers: HashMap::new(),
      clip_duration: clip_duration.unwrap_or(Duration::from_secs(30)),
    }
  }

  pub fn join(&mut self, guild_id: &GuildId) {
    self
      .composers
      .insert(*guild_id, Composer::new(self.clip_duration));
  }

  pub fn leave(&mut self, guild_id: &GuildId) {
    self.composers.remove(guild_id);
  }

  pub fn incoming_tick<'a>(&mut self, guild_id: &GuildId, voices: impl Iterator<Item = &'a [i16]>) {
    let clip_duration = self.clip_duration;
    self
      .composers
      .entry(*guild_id)
      .or_insert_with(|| Composer::new(clip_duration))
      .add_tick(voices);
  }

  pub fn clip(&self, guild_id: &GuildId) -> Vec<i16> {
    match self.composers.get(guild_id) {
      Some(v) => v.compose(),
      None => vec![],
    }
  }

  #[allow(dead_code)]
  pub fn guild_clip_length(&self, guild_id: &GuildId) -> Duration {
    match self.composers.get(guild_id) {
      None => Duration::ZERO,
      Some(v) => v.duration(),
    }
  }
}
