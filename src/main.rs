use serenity::model::prelude::AttachmentType;

mod client;
mod commands;
mod composer;
mod handler;
mod voice;
use {
  composer::Director,
  serenity::model::prelude::GuildId,
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
      let _ = client
        .http
        .get_channel(554690428876881941)
        .await
        .unwrap()
        .guild()
        .unwrap()
        .send_files(
          client.http.clone(),
          vec![AttachmentType::from(std::path::Path::new(&path))],
          |m| m.content("Clip!"),
        )
        .await;
    });

    format!("Clip!")
  });

  warp::serve(clip).run(([127, 0, 0, 1], 3001)).await;
}
