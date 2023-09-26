use {
  crate::voice::GuildVoiceState,
  serenity::model::prelude::GuildId,
  std::{
    collections::{HashMap, LinkedList},
    sync::{Arc, Mutex},
    time::Duration,
  },
};

pub type DirectorImplementation = Arc<Mutex<Director>>;

pub struct Snippet {
  start: u64,
  end: u64,
  data: Vec<i16>,
  sampling_rate: u32,
  track: u32,
}

impl Snippet {
  pub fn new(data: Vec<i16>, sampling_rate: u32, timestamp: u32, track: u32) -> Self {
    let sampling_rate_ms = sampling_rate as u64 / 1000;

    let end = timestamp as u64 + (data.len() as u64 / (sampling_rate_ms * 2));

    Snippet {
      start: timestamp as u64,
      end,
      data,
      sampling_rate,
      track,
    }
  }
}

pub struct Composer {
  snippets: LinkedList<Snippet>,
}

impl Composer {
  pub fn new() -> Self {
    Composer {
      snippets: LinkedList::new(),
    }
  }

  pub fn add_snippet(&mut self, snippet: Snippet) {
    self.snippets.push_back(snippet);
  }

  pub fn shift(&mut self) {
    self.snippets.pop_front();
  }

  pub fn duration(&self) -> Duration {
    let Some(first_snippet) = self.snippets.front() else {
      return Duration::from_secs(0);
    };

    let last_snippet = self.snippets.back().unwrap();

    return Duration::from_millis(last_snippet.end - first_snippet.start);
  }

  pub fn compose(&self) -> Vec<i16> {
    if self.snippets.len() < 1 {
      return Vec::new();
    }

    let first_snippet = self.snippets.front().unwrap();
    let clip_start = first_snippet.start;

    let sampling_rate_ms = first_snippet.sampling_rate as u64 / 1000;

    let mut audio: Vec<i16> =
      vec![0; (self.duration().as_millis() as u64 * sampling_rate_ms * 2) as usize];

    let mut track_map = HashMap::<u32, usize>::new();

    for snippet in self.snippets.iter() {
      let snippet_start_index = match track_map.get(&snippet.track) {
        None => ((snippet.start - clip_start) * sampling_rate_ms * 2) as usize,
        Some(v) => *v,
      };

      for (index, packet) in snippet.data.iter().enumerate() {
        let global_index = snippet_start_index + index;

        // this catches to long audio clips. Too long audio clips can happen if the shifting moved some snippets around. We can ignore this.
        if global_index >= audio.len() {
          // println!("Clip is too long for predetermined clip length. Available indexes: {}, Audio index: {}", audio.len()-1, global_index);
          continue;
        }

        audio[global_index] = (audio[global_index] as i32 + *packet as i32)
          .clamp(i16::MIN as i32, i16::MAX as i32) as i16;

        track_map.insert(snippet.track.clone(), global_index + 1);
      }
    }

    audio
  }
}

pub struct Director {
  composers: HashMap<GuildId, Composer>,
  clip_duration: Duration,
  sampling_rate: u32,
}

impl Director {
  pub fn new(sampling_rate: u32, clip_duration: Option<Duration>) -> Self {
    Director {
      composers: HashMap::new(),
      clip_duration: clip_duration.unwrap_or(Duration::from_secs(30)),
      sampling_rate,
    }
  }

  pub fn handle_guild_voice_state(&mut self, state: &GuildVoiceState) {
    match state {
      GuildVoiceState::Move(guild_id, _) | GuildVoiceState::Join(guild_id, _) => {
        self.join(&guild_id);
      }
      GuildVoiceState::Leave(guild_id) => {
        self.leave(&guild_id);
      }
      _ => {}
    }
  }

  fn join(&mut self, guild_id: &GuildId) {
    self.leave(guild_id);
    self.composers.insert(*guild_id, Composer::new());
  }

  pub fn leave(&mut self, guild_id: &GuildId) {
    self.composers.remove(guild_id);
  }

  pub fn incoming_audio(
    &mut self,
    guild_id: &GuildId,
    incoming_audio: Vec<i16>,
    timestamp: u32,
    track: u32,
  ) {
    let composer = match self.composers.get_mut(guild_id) {
      None => {
        let composer = Composer::new();
        self.composers.insert(guild_id.clone(), composer);
        self.composers.get_mut(guild_id).unwrap()
      }
      Some(v) => v,
    };

    composer.add_snippet(Snippet::new(
      incoming_audio,
      self.sampling_rate,
      timestamp,
      track,
    ));

    println!("{:?}", composer.duration());

    while composer.duration() > self.clip_duration {
      composer.shift();
    }
  }

  pub fn clip(&mut self, guild_id: &GuildId) -> Vec<i16> {
    match self.composers.get(guild_id) {
      Some(v) => v.compose(),
      None => {
        vec![]
      }
    }
  }

  #[allow(dead_code)]
  pub fn guild_clip_length(&mut self, guild_id: &GuildId) -> Duration {
    match self.composers.get(guild_id) {
      None => Duration::ZERO,
      Some(v) => v.duration(),
    }
  }
}
