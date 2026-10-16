//! Minimal runtime bindings to the moonshine streaming speech-to-text C API
//! (https://github.com/moonshine-ai/moonshine, core/moonshine-c-api.h).
//! The library is loaded with dlopen so the bot still runs without it.

use {
  libloading::{Library, Symbol},
  std::{
    ffi::{c_char, CStr, CString},
    path::Path,
  },
};

const MOONSHINE_HEADER_VERSION: i32 = 30000;
pub const SAMPLE_RATE: i32 = 16_000;

#[derive(Clone, Copy, Debug)]
pub enum ModelArch {
  Tiny = 2,
  Small = 4,
  Medium = 5,
}

#[repr(C)]
struct Option_ {
  name: *const c_char,
  value: *const c_char,
}

#[repr(C)]
struct TranscriptLine {
  text: *const c_char,
  audio_data: *const f32,
  audio_data_count: usize,
  start_time: f32,
  duration: f32,
  id: u64,
  is_complete: i8,
  is_updated: i8,
  is_new: i8,
  has_text_changed: i8,
  have_speakers_changed: i8,
  speaker_spans: *const std::ffi::c_void,
  speaker_span_count: u64,
  last_transcription_latency_ms: u32,
  words: *const std::ffi::c_void,
  word_count: u64,
}

#[repr(C)]
struct Transcript {
  lines: *const TranscriptLine,
  line_count: u64,
}

type LoadFn = unsafe extern "C" fn(*const c_char, u32, *const Option_, u64, i32) -> i32;
type ErrorFn = unsafe extern "C" fn(i32) -> *const c_char;
type FreeFn = unsafe extern "C" fn(i32);
type StreamFn = unsafe extern "C" fn(i32, i32) -> i32;
type CreateStreamFn = unsafe extern "C" fn(i32, u32) -> i32;
type KeytermsFn = unsafe extern "C" fn(i32, *const c_char) -> i32;
type AddAudioFn = unsafe extern "C" fn(i32, i32, *const f32, u64, i32, u32) -> i32;
type TranscribeFn = unsafe extern "C" fn(i32, i32, u32, *mut *const Transcript) -> i32;

pub struct Line {
  pub id: u64,
  pub text: String,
  pub is_complete: bool,
}

pub struct Transcriber {
  lib: Library,
  handle: i32,
}

// the C API is documented as thread safe
unsafe impl Send for Transcriber {}
unsafe impl Sync for Transcriber {}

impl Transcriber {
  pub fn load(
    library_path: &Path,
    model_path: &Path,
    arch: ModelArch,
    keyterms: &str,
  ) -> Result<Self, String> {
    let lib = unsafe { Library::new(library_path) }
      .map_err(|e| format!("could not load {}: {}", library_path.display(), e))?;

    let path = CString::new(model_path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    let option_names = [c"keyterms"];
    let keyterms = CString::new(keyterms).map_err(|e| e.to_string())?;
    let options = [Option_ {
      name: option_names[0].as_ptr(),
      value: keyterms.as_ptr(),
    }];
    let options_count = if keyterms.is_empty() { 0 } else { 1 };

    let handle = unsafe {
      let load: Symbol<LoadFn> = lib
        .get(b"moonshine_load_transcriber_from_files")
        .map_err(|e| e.to_string())?;
      load(
        path.as_ptr(),
        arch as u32,
        options.as_ptr(),
        options_count,
        MOONSHINE_HEADER_VERSION,
      )
    };

    let transcriber = Transcriber { lib, handle };
    if handle < 0 {
      return Err(format!(
        "could not load model {}: {}",
        model_path.display(),
        transcriber.error(handle)
      ));
    }
    Ok(transcriber)
  }

  fn error(&self, code: i32) -> String {
    unsafe {
      match self.lib.get::<ErrorFn>(b"moonshine_error_to_string") {
        Ok(f) => {
          let ptr = f(code);
          if ptr.is_null() {
            format!("error {}", code)
          } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
          }
        }
        Err(_) => format!("error {}", code),
      }
    }
  }

  fn check(&self, code: i32) -> Result<i32, String> {
    if code < 0 {
      Err(self.error(code))
    } else {
      Ok(code)
    }
  }

  fn symbol<T>(&self, name: &[u8]) -> Result<Symbol<'_, T>, String> {
    unsafe { self.lib.get(name) }.map_err(|e| e.to_string())
  }

  pub fn set_keyterms(&self, keyterms: &str) -> Result<(), String> {
    let keyterms = CString::new(keyterms).map_err(|e| e.to_string())?;
    let f = self.symbol::<KeytermsFn>(b"moonshine_transcriber_set_keyterms")?;
    self.check(unsafe { f(self.handle, keyterms.as_ptr()) })?;
    Ok(())
  }

  pub fn create_stream(&self) -> Result<i32, String> {
    let create = self.symbol::<CreateStreamFn>(b"moonshine_create_stream")?;
    let stream = self.check(unsafe { create(self.handle, 0) })?;
    let start = self.symbol::<StreamFn>(b"moonshine_start_stream")?;
    self.check(unsafe { start(self.handle, stream) })?;
    Ok(stream)
  }

  pub fn free_stream(&self, stream: i32) {
    if let Ok(stop) = self.symbol::<StreamFn>(b"moonshine_stop_stream") {
      unsafe { stop(self.handle, stream) };
    }
    if let Ok(free) = self.symbol::<StreamFn>(b"moonshine_free_stream") {
      unsafe { free(self.handle, stream) };
    }
  }

  /// `audio` is 16kHz mono PCM in [-1, 1]
  pub fn add_audio(&self, stream: i32, audio: &[f32]) -> Result<(), String> {
    let f = self.symbol::<AddAudioFn>(b"moonshine_transcribe_add_audio_to_stream")?;
    self.check(unsafe {
      f(
        self.handle,
        stream,
        audio.as_ptr(),
        audio.len() as u64,
        SAMPLE_RATE,
        0,
      )
    })?;
    Ok(())
  }

  /// Returns the lines that changed since the last call.
  pub fn transcribe(&self, stream: i32) -> Result<Vec<Line>, String> {
    let f = self.symbol::<TranscribeFn>(b"moonshine_transcribe_stream")?;
    let mut transcript: *const Transcript = std::ptr::null();
    self.check(unsafe { f(self.handle, stream, 0, &mut transcript) })?;

    let mut lines = Vec::new();
    if transcript.is_null() {
      return Ok(lines);
    }

    // the transcript is owned by the transcriber and valid until the next call on it
    unsafe {
      let transcript = &*transcript;
      if transcript.lines.is_null() {
        return Ok(lines);
      }
      for line in std::slice::from_raw_parts(transcript.lines, transcript.line_count as usize) {
        if line.is_updated == 0 || line.text.is_null() {
          continue;
        }
        lines.push(Line {
          id: line.id,
          text: CStr::from_ptr(line.text).to_string_lossy().into_owned(),
          is_complete: line.is_complete != 0,
        });
      }
    }

    Ok(lines)
  }
}

impl Drop for Transcriber {
  fn drop(&mut self) {
    if self.handle >= 0 {
      if let Ok(free) = self.symbol::<FreeFn>(b"moonshine_free_transcriber") {
        unsafe { free(self.handle) };
      }
    }
  }
}
