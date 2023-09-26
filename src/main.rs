mod client;
mod commands;
mod composer;
mod handler;
mod voice;
use {
  composer::Director,
  serenity::model::prelude::{AttachmentType, GuildId},
  std::sync::{Arc, Mutex},
  warp::Filter,
};

#[tokio::main]
async fn main() {
  let director = Arc::new(Mutex::new(Director::new(48_000, None)));

  let client = client::DiscordClient::new(env!("TOKEN"), director)
    .await
    .expect("Error starting discord client.");

  let clip = warp::path!("clip" / u64).map(move |guild_id: u64| {
    let parsed_id = GuildId(guild_id.clone());
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

    format!("Clip!")
  });

  warp::serve(clip).run(([127, 0, 0, 1], 3001)).await;
}
