use crate::composer::{Director, DirectorImplementation};

use serenity::{
  async_trait,
  client::Context,
  model::prelude::{ChannelId, ChannelType, GuildId, VoiceState},
  prelude::EventHandler,
};

use songbird::{CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler, Songbird};

use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum VoiceError {
  GuildNotFound,
  ChannelFetchError,
  MemberFetchError,
  SongbirdInitError,
  SongbirdConnectError,
}

struct Receiver {
  director: DirectorImplementation,
  guild_id: GuildId,
}

impl Receiver {
  pub fn new(guild_id: GuildId, director: DirectorImplementation) -> Self {
    Self { director, guild_id }
  }
}

#[async_trait]
impl VoiceEventHandler for Receiver {
  async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
    use EventContext as Ctx;

    match ctx {
      Ctx::VoicePacket(data) => {
        if let Some(audio) = data.audio {
          /*let mut vec_1 = self.current_audio.lock().unwrap();
          let vec = vec_1.get_mut();
          vec.extend(audio.into_iter());

          println!("{}", vec.len());

          if vec.len() > 2000000 {
            let _ = wav::write(
              wav::Header::new(wav::header::WAV_FORMAT_PCM, 1, 96_000, 16),
              &wav::BitDepth::Sixteen(vec.to_vec()),
              &mut File::create(Path::new("output/audio.wav")).unwrap(),
            );

            panic!("Exceeded length")
          }*/

          let mut director = self.director.lock().unwrap();

          director.incoming_audio(&self.guild_id, audio.clone());

          println!("{}", data.packet.timestamp.0);

          if director.guild_clip_length(&self.guild_id) > std::time::Duration::from_secs(5) {
            let compose = director.clip(&self.guild_id);
            save_wav("output/clip.wav", compose);
            director.leave(&self.guild_id);
          }
        } else {
          println!("Received an audio packet without audio. Is the driver working?");
        }
      }
      _ => {}
    }

    None
  }
}

fn save_wav(path: &str, data: Vec<i16>) {
  let _ = wav::write(
    wav::Header::new(wav::header::WAV_FORMAT_PCM, 2, 48_000, 16),
    &wav::BitDepth::Sixteen(data.to_vec()),
    &mut std::fs::File::create(std::path::Path::new(path)).unwrap(),
  );
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

      if channel_with_members == None && channel_len > 0 {
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
      leave_voice_channel(ctx, guild_id).await?;
      join_voice_channel(ctx, guild_id, channel_id, director.clone()).await?;
    }
  }

  director.lock().unwrap().handle_guild_voice_state(update);

  return Ok(());
}

async fn join_voice_channel(
  ctx: &Context,
  guild_id: &GuildId,
  channel_id: &ChannelId,
  director: Arc<Mutex<Director>>,
) -> Result<(), VoiceError> {
  let manager = create_songbird_manager(ctx).await?;

  let (handler_lock, conn_result) = manager.join(*guild_id, *channel_id).await;

  match conn_result {
    Ok(_) => {
      let mut handler = handler_lock.lock().await;

      handler.add_global_event(
        CoreEvent::VoicePacket.into(),
        Receiver::new(guild_id.clone(), director),
      );

      Ok(())
    }

    Err(_) => Err(VoiceError::SongbirdConnectError),
  }
}

async fn leave_voice_channel(ctx: &Context, guild_id: &GuildId) -> Result<(), VoiceError> {
  let manager = create_songbird_manager(ctx).await?;
  let _ = manager.leave(*guild_id).await;
  Ok(())
}

async fn create_songbird_manager(ctx: &Context) -> Result<Arc<Songbird>, VoiceError> {
  match songbird::get(ctx).await {
    Some(v) => Ok(v.clone()),
    None => Err(VoiceError::SongbirdInitError),
  }
}
