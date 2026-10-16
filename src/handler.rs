use serenity::{
  async_trait,
  client::Context,
  model::{channel::Message, gateway::Ready, guild::Guild, voice::VoiceState},
  prelude::EventHandler,
};

pub struct HandlerManager {
  internal_handlers: Vec<Box<dyn EventHandler>>,
}

impl HandlerManager {
  pub fn new() -> HandlerManager {
    HandlerManager {
      internal_handlers: Vec::new(),
    }
  }

  pub fn add_handler(&mut self, handler: Box<dyn EventHandler>) {
    self.internal_handlers.push(handler);
  }
}

#[async_trait]
impl EventHandler for HandlerManager {
  async fn ready(&self, ctx: Context, data_about_bot: Ready) {
    for handler in self.internal_handlers.iter() {
      handler.ready(ctx.clone(), data_about_bot.clone()).await;
    }
  }

  async fn guild_create(&self, ctx: Context, guild: Guild, is_new: Option<bool>) {
    for handler in self.internal_handlers.iter() {
      handler
        .guild_create(ctx.clone(), guild.clone(), is_new)
        .await;
    }
  }

  async fn message(&self, ctx: Context, msg: Message) {
    for handler in self.internal_handlers.iter() {
      handler.message(ctx.clone(), msg.clone()).await;
    }
  }

  async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
    for handler in self.internal_handlers.iter() {
      handler
        .voice_state_update(ctx.clone(), old.clone(), new.clone())
        .await;
    }
  }
}
