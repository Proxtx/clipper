use {
  crate::composer::{Director, DirectorImplementation},
  rand::Rng,
  serenity::{
    async_trait,
    client::Context,
    model::prelude::{ChannelId, ChannelType, GuildId, VoiceState},
    prelude::EventHandler,
  },
  songbird::{
    error::JoinResult, CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler, Songbird,
  },
  std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
  },
};

#[derive(Debug)]
pub enum VoiceError {
  GuildNotFound,
  ChannelFetchError,
  MemberFetchError,
  SongbirdInitError,
  SongbirdConnectError,
}

#[derive(Clone)]
struct Receiver {
  director: DirectorImplementation,
  guild_id: GuildId,
  track_manager: Arc<Mutex<HashMap<u32, u32>>>,
  instant: Instant,
}

impl Receiver {
  pub fn new(guild_id: GuildId, director: DirectorImplementation) -> Self {
    Self {
      director,
      guild_id,
      track_manager: Arc::new(Mutex::new(HashMap::new())),
      instant: Instant::now(),
    }
  }
}

#[async_trait]
impl VoiceEventHandler for Receiver {
  async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
    use EventContext as Ctx;

    match ctx {
      Ctx::SpeakingUpdate(data) => {
        let mut manager = self.track_manager.lock().unwrap();
        let track_id: u32 = match data.speaking {
          true => {
            let mut rng = rand::thread_rng();
            rng.gen()
          }
          false => 0,
        };
        manager.insert(data.ssrc, track_id);
      }

      Ctx::VoicePacket(data) => {
        if let Some(audio) = data.audio {
          let mut director = self.director.lock().unwrap();

          let track = *self
            .track_manager
            .lock()
            .unwrap()
            .get(&data.packet.ssrc)
            .unwrap_or(&0);

          if track == 0 {
            return None;
          }

          director.incoming_audio(
            &self.guild_id,
            audio.clone(),
            self.instant.elapsed().as_millis() as u32,
            track,
          );
        } else {
          println!("Received an audio packet without audio. Is the driver working?");
        }
      }
      _ => {}
    };

    None
  }
}

pub fn save_clip(guild_id: &GuildId, data: &[i16]) -> String {
  let path = format!(
    "output/{}/{}.wav",
    guild_id,
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_millis()
  );

  let output_dir = format!("output/{}", guild_id);

  std::fs::create_dir_all(output_dir).unwrap();

  let _ = wav::write(
    wav::Header::new(wav::header::WAV_FORMAT_PCM, 2, 48_000, 16),
    &wav::BitDepth::Sixteen(data.to_vec()),
    &mut std::fs::File::create(std::path::Path::new(&path)).unwrap(),
  );

  path
}

pub struct Handler {
  director: Arc<Mutex<Director>>,
}

impl Handler {
  pub fn new(director: Arc<Mutex<Director>>) -> Self {
    Handler { director }
  }
}

#[async_trait]
impl EventHandler for Handler {
  async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
    let voice_update = match new.guild_id {
      Some(id) => get_guild_voice_update(&ctx, id).await,
      None => return,
    };

    let update = match voice_update {
      Ok(v) => v,
      Err(e) => {
        println!("Encountered voice error. {:?}", e);
        return;
      }
    };

    let res = handle_guild_voice_state(&ctx, &update, self.director.clone()).await;

    match res {
      Ok(_) => {}
      Err(err) => {
        println!("{:?}", err);
      }
    }

    println!("Update {:?}", &update)
  }
}

#[derive(Debug)]
pub enum GuildVoiceState {
  Leave(GuildId),
  Join(GuildId, ChannelId),
  Move(GuildId, ChannelId),
  Nothing,
}

async fn get_guild_voice_update(
  ctx: &Context,
  guild_id: GuildId,
) -> Result<GuildVoiceState, VoiceError> {
  let guild = match ctx.http.get_guild(guild_id.0).await {
    Ok(guild) => guild,
    Err(_) => return Err(VoiceError::GuildNotFound),
  };

  let mut channel_with_members: Option<ChannelId> = None;
  let mut move_instead = false;

  let channels = match guild.channels(&ctx.http).await {
    Ok(v) => v,
    Err(_) => return Err(VoiceError::ChannelFetchError),
  };

  'channel_loop: for channel in channels.values() {
    let kind = channel.clone().kind;
    if kind == ChannelType::Voice {
      let members = match channel.members(&ctx.cache).await {
        Err(_) => {
          return Err(VoiceError::MemberFetchError);
        }
        Ok(v) => v,
      };

      let channel_len = members.len();

      for member in members {
        if member.user.id == ctx.cache.current_user().id {
          if channel_len > 1 {
            return Ok(GuildVoiceState::Nothing);
          } else {
            move_instead = true;
            continue 'channel_loop;
          }
        }
      }

      if channel_with_members.is_none() && channel_len > 0 {
        channel_with_members = Some(channel.id);
      }
    }
  }

  match channel_with_members {
    Some(id) => {
      if move_instead {
        Ok(GuildVoiceState::Move(guild_id, id))
      } else {
        Ok(GuildVoiceState::Join(guild_id, id))
      }
    }
    None => {
      if move_instead {
        Ok(GuildVoiceState::Leave(guild_id))
      } else {
        Ok(GuildVoiceState::Nothing)
      }
    }
  }
}

async fn handle_guild_voice_state(
  ctx: &Context,
  update: &GuildVoiceState,
  director: DirectorImplementation,
) -> Result<(), VoiceError> {
  match update {
    GuildVoiceState::Nothing => {}
    GuildVoiceState::Join(guild_id, channel_id) => {
      join_voice_channel(ctx, guild_id, channel_id, director.clone()).await?;
    }

    GuildVoiceState::Leave(guild_id) => {
      leave_voice_channel(ctx, guild_id).await?;
    }

    GuildVoiceState::Move(guild_id, channel_id) => {
      join_voice_channel(ctx, guild_id, channel_id, director.clone()).await?;
    }
  }

  director.lock().unwrap().handle_guild_voice_state(update);

  Ok(())
}

async fn join_voice_channel(
  ctx: &Context,
  guild_id: &GuildId,
  channel_id: &ChannelId,
  director: Arc<Mutex<Director>>,
) -> Result<(), VoiceError> {
  let manager = create_songbird_manager(ctx).await?;

  let (handler_lock, conn_result) = match manager.get(guild_id.0) {
    Some(call_arc) => {
      let mut call = call_arc.lock().await;
      call.remove_all_global_events();
      let result = call.join(*channel_id).await;
      let parsed_result: JoinResult<()> = match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
      };
      (call_arc.clone(), parsed_result)
    }
    None => manager.join(*guild_id, *channel_id).await,
  };

  match conn_result {
    Ok(_) => {
      let mut handler = handler_lock.lock().await;

      match handler.deafen(false).await {
        Ok(_) => {}
        Err(_) => {
          println!("Was unable to set deaf status");
        }
      }

      let receiver = Receiver::new(*guild_id, director);

      handler.add_global_event(CoreEvent::VoicePacket.into(), receiver.clone());
      handler.add_global_event(CoreEvent::SpeakingUpdate.into(), receiver.clone());

      Ok(())
    }

    Err(e) => {
      println!("Error: {}", e);
      Err(VoiceError::SongbirdConnectError)
    }
  }
}

async fn leave_voice_channel(ctx: &Context, guild_id: &GuildId) -> Result<(), VoiceError> {
  let manager = create_songbird_manager(ctx).await?;

  if let Some(call) = manager.get(guild_id.0) {
    match call.lock().await.leave().await {
      Ok(_) => {}
      Err(_) => {
        return Err(VoiceError::SongbirdConnectError);
      }
    }
  };
  Ok(())
}

async fn create_songbird_manager(ctx: &Context) -> Result<Arc<Songbird>, VoiceError> {
  match songbird::serenity::get(ctx).await {
    Some(v) => Ok(v.clone()),
    None => Err(VoiceError::SongbirdInitError),
  }
}
