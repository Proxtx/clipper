use {
  crate::moonshine::{ModelArch, Transcriber},
  serenity::model::prelude::{GuildId, UserId},
  std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError, SyncSender, TrySendError},
    time::{Duration, Instant, SystemTime},
  },
  tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const RELOAD_INTERVAL: Duration = Duration::from_secs(2);
/// silence appended after someone stops talking, so the model closes the line
const TRAILING_SILENCE: Duration = Duration::from_millis(400);
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// a phrase may continue in the next line if the pause is shorter than this
const PHRASE_PAUSE: Duration = Duration::from_secs(5);
/// about 10s of 20ms frames, older audio is dropped if transcription falls behind
const QUEUE_SIZE: usize = 500;

#[derive(Debug)]
pub struct TriggerEvent {
  pub guild_id: GuildId,
  pub user_id: Option<UserId>,
  pub phrase: String,
}

enum Message {
  Audio {
    guild_id: GuildId,
    ssrc: u32,
    user_id: Option<UserId>,
    samples: Vec<f32>,
  },
  LeaveGuild(GuildId),
}

/// Cheap to clone, used by the voice receivers to feed audio to the worker.
#[derive(Clone)]
pub struct TriggerHandle {
  tx: SyncSender<Message>,
}

impl TriggerHandle {
  /// `samples` is 16kHz mono
  pub fn audio(&self, guild_id: GuildId, ssrc: u32, user_id: Option<UserId>, samples: Vec<f32>) {
    let msg = Message::Audio {
      guild_id,
      ssrc,
      user_id,
      samples,
    };
    if let Err(TrySendError::Full(_)) = self.tx.try_send(msg) {
      println!("Trigger detection is falling behind, dropping audio");
    }
  }

  pub fn leave_guild(&self, guild_id: GuildId) {
    let _ = self.tx.try_send(Message::LeaveGuild(guild_id));
  }
}

/// Converts one 20ms frame of 48kHz interleaved stereo into 16kHz mono.
pub fn downsample(stereo_48k: &[i16]) -> Vec<f32> {
  stereo_48k
    .as_chunks::<6>()
    .0
    .iter()
    .map(|c| c.iter().map(|s| *s as f32).sum::<f32>() / (6.0 * 32768.0))
    .collect()
}

pub struct Config {
  pub triggers_file: PathBuf,
  pub library: PathBuf,
  pub model: PathBuf,
  pub arch: ModelArch,
  /// bias the model towards the trigger phrases. Off by default: moonshine v0.1.5
  /// intermittently fails to decode while keyterms are set.
  pub keyterms: bool,
}

impl Config {
  pub fn from_env() -> Self {
    let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.into());
    let model = PathBuf::from(var("MOONSHINE_MODEL", "models/small-streaming-de"));
    let model_name = model.file_name().unwrap_or_default().to_string_lossy();
    let arch = if model_name.starts_with("tiny") {
      ModelArch::Tiny
    } else if model_name.starts_with("medium") {
      ModelArch::Medium
    } else {
      ModelArch::Small
    };

    Config {
      triggers_file: var("TRIGGERS_FILE", "triggers.txt").into(),
      library: var(
        "MOONSHINE_LIB",
        "vendor/moonshine-voice/lib/libmoonshine.so",
      )
      .into(),
      model,
      arch,
      keyterms: std::env::var("TRIGGER_KEYTERMS").is_ok_and(|v| v == "1"),
    }
  }
}

fn keyterms(enabled: bool, triggers: &TriggerList) -> String {
  if enabled {
    triggers.keyterms()
  } else {
    String::new()
  }
}

/// Starts the detection thread. Returns None when the model can't be loaded,
/// in which case the bot runs without trigger words.
pub fn start(config: Config) -> Option<(TriggerHandle, UnboundedReceiver<TriggerEvent>)> {
  let mut triggers = TriggerList::new(config.triggers_file);
  triggers.reload();

  let transcriber = match Transcriber::load(
    &config.library,
    &config.model,
    config.arch,
    &keyterms(config.keyterms, &triggers),
  ) {
    Ok(v) => v,
    Err(e) => {
      println!("Trigger words disabled: {}", e);
      return None;
    }
  };

  println!(
    "Trigger words enabled ({:?} model {}), {} trigger(s) in {}",
    config.arch,
    config.model.display(),
    triggers.phrases.len(),
    triggers.path.display()
  );

  let (tx, rx) = mpsc::sync_channel(QUEUE_SIZE);
  let (event_tx, event_rx) = unbounded_channel();

  std::thread::Builder::new()
    .name("trigger-words".into())
    .spawn(move || {
      Worker {
        transcriber,
        triggers,
        keyterms: config.keyterms,
        streams: HashMap::new(),
        events: event_tx,
        debug: std::env::var("TRIGGER_DEBUG").is_ok(),
      }
      .run(rx)
    })
    .expect("Was unable to spawn trigger thread");

  Some((TriggerHandle { tx }, event_rx))
}

struct Trigger {
  phrase: String,
  normalized: String,
}

struct TriggerList {
  path: PathBuf,
  modified: Option<SystemTime>,
  phrases: Vec<Trigger>,
}

impl TriggerList {
  fn new(path: PathBuf) -> Self {
    TriggerList {
      path,
      modified: None,
      phrases: Vec::new(),
    }
  }

  /// Re-reads the file if it changed. Returns true if the triggers changed.
  fn reload(&mut self) -> bool {
    let modified = std::fs::metadata(&self.path)
      .and_then(|m| m.modified())
      .ok();
    if modified == self.modified {
      return false;
    }
    self.modified = modified;
    self.phrases = read_triggers(&self.path);
    println!(
      "Loaded {} trigger(s) from {}",
      self.phrases.len(),
      self.path.display()
    );
    true
  }

  fn keyterms(&self) -> String {
    self
      .phrases
      .iter()
      .map(|t| t.phrase.replace(',', " "))
      .collect::<Vec<_>>()
      .join(",")
  }

  fn find(&self, text: &str) -> Option<&Trigger> {
    let text = normalize(text);
    let words: Vec<&str> = text.split(' ').collect();
    self.phrases.iter().find(|t| {
      let trigger: Vec<&str> = t.normalized.split(' ').collect();
      words
        .windows(trigger.len())
        .any(|window| phrase_matches(&trigger, window))
    })
  }
}

/// One trigger per line, empty lines and lines starting with # are ignored.
fn read_triggers(path: &Path) -> Vec<Trigger> {
  let Ok(content) = std::fs::read_to_string(path) else {
    return Vec::new();
  };

  content
    .lines()
    .map(str::trim)
    .filter(|l| !l.is_empty() && !l.starts_with('#'))
    .filter_map(|l| {
      let normalized = normalize(l);
      (!normalized.is_empty()).then(|| Trigger {
        phrase: l.to_string(),
        normalized,
      })
    })
    .collect()
}

/// Speech recognition mishears words, especially names it doesn't know
/// ("Garmin" comes out as "Kamin" or "Gummi"). In phrases of three or more
/// words, one word may differ as long as it starts with a similar sound.
fn phrase_matches(trigger: &[&str], heard: &[&str]) -> bool {
  let mut misheard = 0;
  for (expected, word) in trigger.iter().zip(heard) {
    if expected != word {
      if trigger.len() < 3 || sound_class(expected) != sound_class(word) {
        return false;
      }
      misheard += 1;
    }
  }
  misheard <= 1
}

/// Rough phonetic class of the first letter, after the cologne phonetics
/// (Kölner Phonetik), so that e.g. g/k/c or d/t count as the same sound.
fn sound_class(word: &str) -> u8 {
  match word.chars().next() {
    Some('b' | 'p') => 1,
    Some('d' | 't') => 2,
    Some('f' | 'v' | 'w') => 3,
    Some('c' | 'g' | 'k' | 'q' | 'x') => 4,
    Some('l') => 5,
    Some('m' | 'n') => 6,
    Some('r') => 7,
    Some('s' | 'z' | 'ß') => 8,
    Some(c) if c.is_alphabetic() => 0,
    Some(c) => c as u8,
    None => u8::MAX,
  }
}

/// Lowercase words separated by single spaces, punctuation removed and
/// spellings the model varies on unified ("ok", "o.k." and "okay").
fn normalize(text: &str) -> String {
  let lowercase = text
    .chars()
    .map(|c| if c.is_alphanumeric() { c } else { ' ' })
    .flat_map(char::to_lowercase)
    .collect::<String>();

  let mut words: Vec<&str> = Vec::new();
  for word in lowercase.split_whitespace() {
    match word {
      "ok" | "okey" => words.push("okay"),
      "k" if words.last() == Some(&"o") => *words.last_mut().unwrap() = "okay",
      _ => words.push(word),
    }
  }
  words.join(" ")
}

struct SpeakerStream {
  stream: i32,
  user_id: Option<UserId>,
  last_audio: Instant,
  dirty: bool,
  padded: bool,
  /// lines that already fired, so a growing line only triggers once
  fired: HashSet<u64>,
  /// the last completed line if it didn't fire, so a phrase that the model
  /// splits into two lines because of a pause is still found
  previous: Option<(String, Instant)>,
}

struct Worker {
  transcriber: Transcriber,
  triggers: TriggerList,
  keyterms: bool,
  streams: HashMap<(GuildId, u32), SpeakerStream>,
  events: UnboundedSender<TriggerEvent>,
  debug: bool,
}

impl Worker {
  fn run(mut self, rx: mpsc::Receiver<Message>) {
    let mut last_poll = Instant::now();
    let mut last_reload = Instant::now();

    loop {
      match rx.recv_timeout(POLL_INTERVAL) {
        Ok(msg) => self.handle(msg),
        Err(RecvTimeoutError::Timeout) => {}
        Err(RecvTimeoutError::Disconnected) => break,
      }
      while let Ok(msg) = rx.try_recv() {
        self.handle(msg);
      }

      if last_reload.elapsed() >= RELOAD_INTERVAL {
        last_reload = Instant::now();
        if self.triggers.reload() && self.keyterms {
          if let Err(e) = self.transcriber.set_keyterms(&self.triggers.keyterms()) {
            println!("Was unable to update trigger keyterms: {}", e);
          }
        }
      }

      if last_poll.elapsed() >= POLL_INTERVAL {
        last_poll = Instant::now();
        self.poll();
      }
    }
  }

  fn handle(&mut self, msg: Message) {
    match msg {
      Message::Audio {
        guild_id,
        ssrc,
        user_id,
        samples,
      } => {
        if self.triggers.phrases.is_empty() {
          return;
        }

        let key = (guild_id, ssrc);
        if !self.streams.contains_key(&key) {
          match self.transcriber.create_stream() {
            Ok(stream) => {
              self.streams.insert(
                key,
                SpeakerStream {
                  stream,
                  user_id,
                  last_audio: Instant::now(),
                  dirty: false,
                  padded: false,
                  fired: HashSet::new(),
                  previous: None,
                },
              );
            }
            Err(e) => {
              println!("Was unable to create transcription stream: {}", e);
              return;
            }
          }
        }

        let speaker = self.streams.get_mut(&key).unwrap();
        if user_id.is_some() {
          speaker.user_id = user_id;
        }
        if let Err(e) = self.transcriber.add_audio(speaker.stream, &samples) {
          println!("Was unable to add audio to stream: {}", e);
        }
        speaker.last_audio = Instant::now();
        speaker.dirty = true;
        speaker.padded = false;
      }
      Message::LeaveGuild(guild_id) => {
        let transcriber = &self.transcriber;
        self.streams.retain(|(guild, _), speaker| {
          let keep = *guild != guild_id;
          if !keep {
            transcriber.free_stream(speaker.stream);
          }
          keep
        });
      }
    }
  }

  fn poll(&mut self) {
    let silence = vec![0.0; (TRAILING_SILENCE.as_millis() as usize) * 16];

    for ((guild_id, _), speaker) in self.streams.iter_mut() {
      // discord stops sending packets when someone stops talking
      if !speaker.padded && speaker.last_audio.elapsed() >= TRAILING_SILENCE {
        speaker.padded = true;
        speaker.dirty = true;
        let _ = self.transcriber.add_audio(speaker.stream, &silence);
      }

      if !speaker.dirty {
        continue;
      }
      speaker.dirty = false;

      let lines = match self.transcriber.transcribe(speaker.stream) {
        Ok(v) => v,
        Err(e) => {
          println!("Transcription failed: {}", e);
          continue;
        }
      };

      for line in lines {
        if self.debug && line.is_complete {
          println!("Heard: \"{}\"", line.text);
        }
        let text = match &speaker.previous {
          Some((previous, at)) if at.elapsed() < PHRASE_PAUSE => {
            format!("{} {}", previous, line.text)
          }
          _ => line.text.clone(),
        };

        if !speaker.fired.contains(&line.id) {
          if let Some(trigger) = self.triggers.find(&text) {
            println!("Heard trigger \"{}\" in \"{}\"", trigger.phrase, text);
            speaker.fired.insert(line.id);
            let _ = self.events.send(TriggerEvent {
              guild_id: *guild_id,
              user_id: speaker.user_id,
              phrase: trigger.phrase.clone(),
            });
          }
        }
        // completed lines never change again
        if line.is_complete {
          speaker.previous = if speaker.fired.remove(&line.id) {
            None
          } else {
            Some((line.text, Instant::now()))
          };
        }
      }
    }

    let transcriber = &self.transcriber;
    self.streams.retain(|_, speaker| {
      let keep = speaker.last_audio.elapsed() < STREAM_IDLE_TIMEOUT;
      if !keep {
        transcriber.free_stream(speaker.stream);
      }
      keep
    });
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn list(phrases: &[&str]) -> TriggerList {
    let mut list = TriggerList::new(PathBuf::new());
    list.phrases = phrases
      .iter()
      .map(|p| Trigger {
        phrase: p.to_string(),
        normalized: normalize(p),
      })
      .collect();
    list
  }

  #[test]
  fn matches_whole_words_ignoring_case_and_punctuation() {
    let triggers = list(&["Clip that", "Grüße"]);
    assert_eq!(
      triggers.find("Okay, clip that!").map(|t| t.phrase.as_str()),
      Some("Clip that")
    );
    assert_eq!(
      triggers.find("liebe Grüße.").map(|t| t.phrase.as_str()),
      Some("Grüße")
    );
    assert!(triggers.find("eclipse that").is_none());
  }

  #[test]
  fn ok_and_okay_are_the_same() {
    let triggers = list(&["ok garmin video speichern"]);
    assert!(triggers.find("Okay, Garmin, Video speichern.").is_some());
    assert!(triggers.find("O.K. Garmin Video speichern").is_some());
  }

  #[test]
  fn one_similar_sounding_word_may_be_misheard() {
    let triggers = list(&["ok garmin video speichern"]);
    // what the german model makes of "Garmin"
    assert!(triggers.find("Okay, Kamin. Video speichern.").is_some());
    assert!(triggers.find("Okay, Gummi, Video speichern.").is_some());
    // a different command must not match
    assert!(triggers.find("Okay Garmin, Video löschen.").is_none());
    // only one word may differ
    assert!(triggers.find("Okay Kamin, Bild speichern.").is_none());
    // short phrases must match exactly
    assert!(list(&["hallo welt"]).find("hallo wald").is_none());
  }

  #[test]
  fn keyterms_are_comma_separated() {
    assert_eq!(list(&["a, b", "c"]).keyterms(), "a  b,c");
  }

  #[test]
  fn downsample_averages_channels_and_samples() {
    let frame = [32767i16; 1920];
    let out = downsample(&frame);
    assert_eq!(out.len(), 320);
    assert!((out[0] - 1.0).abs() < 0.001);
  }
}

/// Runs the real model: TEST_WAV is a recording containing TEST_TRIGGER.
/// MOONSHINE_LIB and MOONSHINE_MODEL work like in the bot.
#[cfg(test)]
mod model_tests {
  use super::*;

  #[test]
  #[ignore = "needs the moonshine library, a model and TEST_WAV / TEST_TRIGGER"]
  fn detects_trigger_in_recording() {
    let trigger = std::env::var("TEST_TRIGGER").unwrap();
    let triggers_file = std::env::temp_dir().join("clipper-test-triggers.txt");
    std::fs::write(&triggers_file, format!("# test\n{}\n", trigger)).unwrap();

    let mut reader = hound::WavReader::open(std::env::var("TEST_WAV").unwrap()).unwrap();
    let spec = reader.spec();
    let mono: Vec<f32> = reader
      .samples::<i16>()
      .map(|s| s.unwrap() as f32 / 32768.0)
      .collect::<Vec<_>>()
      .chunks(spec.channels as usize)
      .map(|c| c.iter().sum::<f32>() / c.len() as f32)
      .collect();
    // linear resampling to 16kHz
    let step = spec.sample_rate as f32 / 16_000.0;
    let resampled: Vec<f32> = (0..(mono.len() as f32 / step) as usize)
      .map(|i| {
        let pos = i as f32 * step;
        let (index, frac) = (pos as usize, pos.fract());
        let next = mono.get(index + 1).copied().unwrap_or(mono[index]);
        mono[index] * (1.0 - frac) + next * frac
      })
      .collect();

    let config = Config {
      triggers_file,
      ..Config::from_env()
    };
    let (handle, mut events) = start(config).expect("model should load");

    let audio_length = Duration::from_secs_f32(resampled.len() as f32 / 16_000.0);
    let started = Instant::now();
    // feed in real time from another thread, like the voice receiver does
    std::thread::spawn(move || {
      for frame in resampled.chunks(320) {
        handle.audio(GuildId::new(1), 42, None, frame.to_vec());
        std::thread::sleep(Duration::from_millis(20));
      }
      std::thread::sleep(Duration::from_secs(10));
    });

    let event = tokio::runtime::Runtime::new()
      .unwrap()
      .block_on(async { tokio::time::timeout(Duration::from_secs(10), events.recv()).await })
      .expect("trigger should fire within 10s")
      .unwrap();
    println!(
      "fired \"{}\" {:?} into a {:?} recording",
      event.phrase,
      started.elapsed(),
      audio_length
    );
    assert_eq!(event.phrase, trigger);
  }
}
