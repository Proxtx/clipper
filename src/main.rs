mod client;
mod commands;
mod composer;
mod handler;
mod voice;
use {
  composer::Director,
  serenity::model::prelude::{AttachmentType, GuildId},
  std::{
    env,
    sync::{Arc, Mutex},
    time::Duration,
  },
  warp::Filter,
};

#[tokio::main]
async fn main() {
  let clip_duration: Option<Duration> = match env::var("DURATION") {
    Ok(v) => Some(Duration::from_millis(
      v.parse().expect("Was unable to parse duration!"),
    )),
    Err(_) => None,
  };

  let director = Arc::new(Mutex::new(Director::new(48_000, clip_duration)));

  let client = client::DiscordClient::new(
    &env::var("TOKEN").expect("TOKEN was not provided!"),
    director,
  )
  .await
  .expect("Error starting discord client.");

  let clip = warp::path!("clip" / u64).map(move |guild_id: u64| {
    let parsed_id = GuildId(guild_id);
    let data = client.director.lock().unwrap().clip(&parsed_id);

    let path = voice::save_clip(&parsed_id, &data);

    let client = client.client.clone();

    tokio::spawn(async move {
      for (_channel_id, guild_channel) in client
        .http
        .get_guild(guild_id)
        .await?
        .channels(client.http.clone())
        .await?
      {
        if guild_channel.name == "clips" {
          guild_channel
            .send_files(
              client.http.clone(),
              vec![AttachmentType::from(std::path::Path::new(&path))],
              |m| m.content("Clip!"),
            )
            .await?;
        }
      }

      Ok::<(), serenity::Error>(())
    });

    "Clip!"
  });

  warp::serve(clip)
    .run((
      [127, 0, 0, 1],
      env::var("PORT")
        .expect("PORT was not provided!")
        .parse()
        .expect("Port is not a number"),
    ))
    .await;
}
