use crate::voice::GuildVoiceState;
use serenity::model::prelude::GuildId;
use std::collections::{HashMap, LinkedList};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type DirectorImplementation = Arc<Mutex<Director>>;

pub struct Snippet {
  start: SystemTime,
  end: SystemTime,
  data: Vec<i16>,
  sampling_rate: u32,
}

impl Snippet {
  pub fn new(data: Vec<i16>, sampling_rate: u32) -> Self {
    let sampling_rate_ms = sampling_rate as u128 / 1000;

    let start = SystemTime::now();
    let end = start
      + Duration::from_millis(
        u64::try_from(data.len() as u128 / (sampling_rate_ms * 2))
          .expect("Duration calculation overflow error!"),
      );

    Snippet {
      start,
      end,
      data,
      sampling_rate,
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

    return last_snippet
      .end
      .duration_since(first_snippet.start)
      .expect("Time travel??");
  }

  pub fn compose(&self) -> Vec<i16> {
    if self.snippets.len() < 1 {
      return Vec::new();
    }

    let mut last_snippet_end: SystemTime = UNIX_EPOCH;

    for snippet in self.snippets.iter() {
      println!(
        "Coherent: {}",
        snippet
          .start
          .duration_since(UNIX_EPOCH)
          .unwrap()
          .as_millis() as i128
          - last_snippet_end
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i128
      );
      last_snippet_end = snippet.end;
    }

    let first_snippet = self.snippets.front().unwrap();
    let clip_start = first_snippet.start;

    let sampling_rate_ms = first_snippet.sampling_rate as u128 / 1000;

    let mut audio: Vec<i16> =
      vec![0; (self.duration().as_millis() * sampling_rate_ms * 2) as usize];

    let mut last_index = 0;

    for snippet in self.snippets.iter() {
      let mut snippet_start_index = snippet
        .start
        .duration_since(clip_start)
        .unwrap()
        .as_millis()
        * sampling_rate_ms
        * 2;

      if snippet_start_index % 2 == 1 {
        snippet_start_index += 1;
      }

      //println!("{}", snippet_start_index as i128 - last_index);

      for (index, sample) in snippet.data.iter().enumerate() {
        let index = snippet_start_index as usize + index;

        audio[index] = (i32::clamp(
          *sample as i32 + audio[index] as i32,
          i16::MIN as i32,
          i16::MAX as i32,
        )) as i16;

        last_index = index as i128;
      }
    }

    audio

    /*if self.snippets.len() < 1 {
      return Vec::new();
    }

    let first_snippet = self.snippets.front().unwrap();
    let clip_start = first_snippet.start;
    let sampling_rate_ms = first_snippet.sampling_rate as u128 / 1000;

    println!(
      "{} {}",
      sampling_rate_ms,
      first_snippet.sampling_rate as f64 / 1000 as f64
    );

    let mut long_snippet: Vec<i16> =
      vec![0; (self.duration().as_millis() * sampling_rate_ms) as usize];

    println!("{}", long_snippet.len());

    let mut last_index = 0;

    for snippet in self.snippets.iter() {
      let clip_start_index = snippet
        .start
        .duration_since(clip_start)
        .unwrap()
        .as_millis()
        * sampling_rate_ms;*/

    /*println!(
      "Indexes since start: {}",
      clip_start_index as i128 - last_index as i128
    );

    if (clip_start_index as i128 - last_index as i128) < 1 {
      continue;
    }*/

    /*let clip_start_distance = snippet
    .start
    .duration_since(first_snippet.start)
    .unwrap()
    .as_millis();*/

    //for (index, sample) in snippet.data.iter().enumerate() {
    /*let own_duration = index / snippet.sampling_rate as usize;
    println!("{}", index as f32 / snippet.sampling_rate as f32);
    let index =
      (clip_start_distance as usize + own_duration) as usize / snippet.sampling_rate as usize;*/

    //let global_index = index as u128 + clip_start_index;

    /*if global_index as usize >= long_snippet.len() {
      println!("global index out of range {}", global_index);
      continue;
    }*/

    //println!("{}", global_index);

    //let own_duration = index;

    /*long_snippet[global_index as usize] = i32::clamp(
      long_snippet[global_index as usize] as i32 + *sample as i32,
      i16::MIN as i32,
      i16::MAX as i32,
    ) as i16;

    last_index = global_index;*/

    //println!("{} {}", long_snippet[index], index)
    //}
    //long_snippet.extend(snippet.data.iter());
    //}

    //long_snippet
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

  pub fn incoming_audio(&mut self, guild_id: &GuildId, incoming_audio: Vec<i16>) {
    let composer = match self.composers.get_mut(guild_id) {
      None => {
        let composer = Composer::new();
        self.composers.insert(guild_id.clone(), composer);
        self.composers.get_mut(guild_id).unwrap()
      }
      Some(v) => v,
    };

    composer.add_snippet(Snippet::new(incoming_audio, self.sampling_rate));

    /*println!(
      "Clip from Guild {} has duration {:?}",
      guild_id.clone(),
      composer.duration()
    );*/

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

  pub fn guild_clip_length(&mut self, guild_id: &GuildId) -> Duration {
    match self.composers.get(guild_id) {
      None => Duration::ZERO,
      Some(v) => v.duration(),
    }
  }
}
