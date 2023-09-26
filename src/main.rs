mod client;
mod commands;
mod composer;
mod handler;
mod voice;
use {serenity::model::prelude::GuildId, warp::Filter};

#[tokio::main]
async fn main() {
  let client = client::DiscordClient::new(env!("TOKEN"))
    .await
    .expect("Error starting discord client.");

  let director = client.director;

  let clip = warp::path!("clip" / u64).map(move |guild_id: u64| {
    let parsed_id = GuildId(guild_id.clone());
    let data = director.lock().unwrap().clip(&parsed_id);

    voice::save_clip(&parsed_id, &data);

    format!("Received!")
  });

  warp::serve(clip).run(([127, 0, 0, 1], 3001)).await;
}
