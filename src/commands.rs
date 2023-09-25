use serenity::{
  client::Context,
  framework::standard::{
    macros::{command, group},
    CommandResult,
  },
  model::channel::Message,
};

#[group]
#[commands(info)]
struct General;

#[command]
async fn info(ctx: &Context, msg: &Message) -> CommandResult {
  msg.reply(ctx, "Bot is running!").await?;
  Ok(())
}
