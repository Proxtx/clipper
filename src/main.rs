use serenity::async_trait;
use serenity::prelude::*;
use serenity::model::{channel::Message, prelude::Ready};
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{StandardFramework, CommandResult};
use serenity::model::id::{ChannelId, GuildId};
use serenity::model::prelude::VoiceState;

#[group]
#[commands(clip)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }

    async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        join(&ctx, new.guild_id.unwrap(), new.channel_id.unwrap()).await;
        println!("Voice State update {}", new.user_id)
    }
}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("~")) // set the bot's prefix to "~"
        .group(&GENERAL_GROUP);

    // Login with a bot token from the environment
    let token = env!("TOKEN");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[command]
async fn clip(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Test").await?;

    Ok(())
}

async fn join (ctx: &Context, guild_id: GuildId, channel_id: ChannelId) {

    let manager = songbird::get(ctx).await.expect("Error initializing Songbird").clone();

    let (handler_lock, conn_result) = manager.join(guild_id, channel_id).await;

}