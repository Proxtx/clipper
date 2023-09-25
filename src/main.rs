#![feature(async_closure)]

mod client;
mod commands;
mod handler;
mod voice;

#[tokio::main]
async fn main() {
    let _client = client::DiscordClient::new(env!("TOKEN")).await;
}
