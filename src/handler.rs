use serenity::async_trait;
use serenity::client::Context;
use serenity::model::gateway::Ready;
use serenity::model::voice::VoiceState;
use serenity::prelude::EventHandler;

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

    /*async fn for_each<'a, F, T>(&mut self, function: F)
    where
        F: Fn(Box<dyn EventHandler>) -> T,
        T: Future<Output = &'a Box<dyn EventHandler>> + Send + 'static,
    {
        let old_handlers = std::mem::replace(
            &mut self.internal_handlers,
            Vec::<Box<dyn EventHandler>>::new(),
        );

        let iterator = old_handlers.into_iter();

        for handler in iterator {
            self.internal_handlers
                .push(function(Box::new(*handler + 'static)).await);
        }
    }*/

    /*async fn for_each<F, T>(&self, function: F)
    where
        F: Fn(Box<dyn EventHandler>) -> T,
        T: Future<Output = ()> + Send + 'static,
    {
    }*/
}

/*#[async_trait]
impl EventHandler for HandlerManager {
    async fn ready(&self, ctx: Context, data_about_bot: Ready) {
        let ctx = ctx.clone();
        let data_about_bot = data_about_bot.clone();
        self.for_each(async move |handler: Box<dyn EventHandler>| {
            handler.ready(, data_about_bot.clone()).await;
        })
        .await;
    }
}*/

/*#[async_trait]
impl EventHandler for HandlerManager {
    async fn ready(&self, ctx: Context, data_about_bot: Ready) {
        self.for_each(async move |handler: Box<dyn EventHandler>| {
            handler.ready(ctx, data_about_bot).await;
            &handler
        })
        .await;
    }
}*/

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
