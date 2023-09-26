use serenity::{
  async_trait,
  client::Context,
  model::{gateway::Ready, voice::VoiceState},
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

  async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
    for handler in self.internal_handlers.iter() {
      handler
        .voice_state_update(ctx.clone(), old.clone(), new.clone())
        .await;
    }
  }
}
