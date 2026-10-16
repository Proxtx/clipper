mod client;
mod commands;
mod composer;
mod handler;
mod moonshine;
mod sound;
mod triggers;
mod voice;
use {
  composer::Director,
  serenity::{
    all::{CreateAllowedMentions, CreateAttachment, CreateMessage},
    model::prelude::GuildId,
  },
  sound::Sound,
  std::{
    collections::HashMap,
    env,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
  },
  tokio::sync::mpsc::UnboundedReceiver,
  triggers::TriggerEvent,
  warp::Filter,
};

/// minimum time between two trigger clips in the same guild
const TRIGGER_COOLDOWN: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() {
  let clip_duration: Option<Duration> = match env::var("DURATION") {
    Ok(v) => Some(Duration::from_millis(
      v.parse().expect("Was unable to parse duration!"),
    )),
    Err(_) => None,
  };

  let port: u16 = env::var("PORT")
    .expect("PORT was not provided!")
    .parse()
    .expect("Port is not a number");

  let director = Arc::new(Mutex::new(Director::new(clip_duration)));

  let (trigger, trigger_events) = match triggers::start(triggers::Config::from_env()) {
    Some((handle, events)) => (Some(handle), Some(events)),
    None => (None, None),
  };

  let client = Arc::new(
    client::DiscordClient::new(
      &env::var("TOKEN").expect("TOKEN was not provided!"),
      director,
      trigger,
    )
    .await
    .expect("Error starting discord client."),
  );

  if let Some(events) = trigger_events {
    tokio::spawn(handle_triggers(client.clone(), events));
  }

  let clip = warp::path!("clip" / u64).map(move |guild_id: u64| {
    let parsed_id = GuildId::new(guild_id);
    let client = client.clone();

    tokio::spawn(async move {
      if let Err(e) = post_clip(&client, parsed_id, CreateMessage::new().content("Clip!")).await {
        println!("Was unable to post clip for {}: {}", parsed_id, e);
      }
    });

    "Clip!"
  });

  println!("Starting Webserver");

  warp::serve(clip).run(([127, 0, 0, 1], port)).await;
}

async fn handle_triggers(
  client: Arc<client::DiscordClient>,
  mut events: UnboundedReceiver<TriggerEvent>,
) {
  let sound = Sound::from_env();
  let mut last_trigger: HashMap<GuildId, Instant> = HashMap::new();

  while let Some(event) = events.recv().await {
    if let Some(last) = last_trigger.get(&event.guild_id) {
      if last.elapsed() < TRIGGER_COOLDOWN {
        continue;
      }
    }
    last_trigger.insert(event.guild_id, Instant::now());

    if let Some(call) = client.songbird.get(event.guild_id) {
      call.lock().await.play_input(sound.input());
    }

    let speaker = match event.user_id {
      Some(id) => format!("<@{}>", id),
      None => "someone".into(),
    };
    let message = CreateMessage::new()
      .content(format!("Clip! {} said \"{}\"", speaker, event.phrase))
      .allowed_mentions(CreateAllowedMentions::new());

    if let Err(e) = post_clip(&client, event.guild_id, message).await {
      println!("Was unable to post clip for {}: {}", event.guild_id, e);
    }
  }
}

async fn post_clip(
  client: &client::DiscordClient,
  guild_id: GuildId,
  message: CreateMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let data = client.director.lock().unwrap().clip(&guild_id);
  let path = voice::save_clip(&guild_id, &data)?;

  let mut posted = false;
  for guild_channel in guild_id.channels(&client.http).await?.values() {
    if guild_channel.name == "clips" {
      guild_channel
        .send_message(
          &client.http,
          message
            .clone()
            .add_file(CreateAttachment::path(&path).await?),
        )
        .await?;
      posted = true;
    }
  }

  if !posted {
    println!(
      "No #clips channel found in {}, clip saved to {}",
      guild_id, path
    );
  }

  Ok(())
}
