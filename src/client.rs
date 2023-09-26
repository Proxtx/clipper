use {
  crate::{
    composer::{Director, DirectorImplementation},
    handler::HandlerManager,
    voice::Handler as VoiceHandler,
  },
  serenity::{
    async_trait,
    framework::StandardFramework,
    model::prelude::Ready,
    prelude::{Client, Context, EventHandler, GatewayIntents},
  },
  songbird::{driver::DecodeMode, Config, SerenityInit},
  std::sync::{Arc, Mutex},
};

#[derive(Debug)]
pub enum DiscordClientError {
  ClientCreation,
  ClientConnection,
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
}

impl DiscordClient {
  pub async fn new(token: &str) -> Result<DiscordClient, DiscordClientError> {
    // This is necessary to not run into a runtime error. Don't ask me why
    tracing_subscriber::fmt::init();

    let mut handler_manager = HandlerManager::new();

    let director = Arc::new(Mutex::new(Director::new(48_000, None)));

    handler_manager.add_handler(Box::from(Handler));
    handler_manager.add_handler(Box::from(VoiceHandler::new(director.clone())));

    let intents = GatewayIntents::all();
    let songbird_config = Config::default().decode_mode(DecodeMode::Decode);

    // remove this since framework is legacy
    let framework = StandardFramework::new()
      .configure(|c| c.prefix("~"))
      .group(&crate::commands::GENERAL_GROUP);

    let mut client = match Client::builder(token, intents)
      .event_handler(handler_manager)
      .register_songbird_from_config(songbird_config)
      .framework(framework)
      .await
    {
      Ok(client) => client,
      Err(_) => return Err(DiscordClientError::ClientCreation),
    };

    match client.start().await {
      Ok(_) => {}
      Err(_) => return Err(DiscordClientError::ClientConnection),
    }

    Ok(DiscordClient { director })
  }
}
