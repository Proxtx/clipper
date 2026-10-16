use serenity::{async_trait, client::Context, model::channel::Message, prelude::EventHandler};

pub const PREFIX: &str = "~";

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
  async fn message(&self, ctx: Context, msg: Message) {
    if msg.author.bot {
      return;
    }

    if msg.content.trim() == format!("{}info", PREFIX) {
      if let Err(e) = msg.reply(&ctx, "Bot is running!").await {
        println!("Was unable to reply to info command: {}", e);
      }
    }
  }
}
