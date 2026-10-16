use {
  crate::{
    composer::{DirectorImplementation, SAMPLING_RATE},
    handler::HandlerManager,
    triggers::TriggerHandle,
    voice::Handler as VoiceHandler,
  },
  serenity::{
    all::{Cache, Http},
    async_trait,
    model::prelude::Ready,
    prelude::{Client, Context, EventHandler, GatewayIntents},
  },
  songbird::{
    driver::{Channels, DecodeConfig, DecodeMode, SampleRate},
    Config, SerenityInit, Songbird,
  },
  std::sync::Arc,
};

#[derive(Debug)]
#[allow(dead_code)] // fields are only read through Debug when startup fails
pub enum DiscordClientError {
  ClientCreation(Box<serenity::Error>),
}

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, _ctx: Context, data_about_bot: Ready) {
    println!("{} is ready!", data_about_bot.user.name);
  }
}

pub struct DiscordClient {
  pub director: DirectorImplementation,
  pub http: Arc<Http>,
  pub songbird: Arc<Songbird>,
  #[allow(dead_code)]
  pub cache: Arc<Cache>,
}

impl DiscordClient {
  pub async fn new(
    token: &str,
    director: DirectorImplementation,
    trigger: Option<TriggerHandle>,
  ) -> Result<DiscordClient, DiscordClientError> {
    let mut handler_manager = HandlerManager::new();

    handler_manager.add_handler(Box::from(Handler));
    handler_manager.add_handler(Box::from(crate::commands::Handler));
    handler_manager.add_handler(Box::from(VoiceHandler::new(director.clone(), trigger)));

    // MESSAGE_CONTENT is privileged and only needed for the ~info command
    let intents = GatewayIntents::GUILDS
      | GatewayIntents::GUILD_VOICE_STATES
      | GatewayIntents::GUILD_MESSAGES
      | GatewayIntents::MESSAGE_CONTENT;

    assert_eq!(SAMPLING_RATE, 48_000);
    let songbird = Songbird::serenity_from_config(Config::default().decode_mode(
      DecodeMode::Decode(DecodeConfig::new(Channels::Stereo, SampleRate::Hz48000)),
    ));

    let mut client = Client::builder(token, intents)
      .event_handler(handler_manager)
      .register_songbird_with(songbird.clone())
      .await
      .map_err(|e| DiscordClientError::ClientCreation(Box::new(e)))?;

    let http = client.http.clone();
    let cache = client.cache.clone();

    tokio::spawn(async move {
      if let Err(e) = client.start().await {
        println!("Was unable to start client! {}", e);
        std::process::exit(1);
      }
    });

    Ok(DiscordClient {
      director,
      http,
      songbird,
      cache,
    })
  }
}
