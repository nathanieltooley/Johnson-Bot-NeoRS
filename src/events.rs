use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, GuildId, Message, Ready, Result};

use rand::Rng;
use regex::Regex;
use tracing::{debug, error, info, instrument};

use crate::checks::slurs;
use crate::custom_types::command::SerenityCtxData;
use crate::mongo::ContextWrapper;
pub struct Handler;

const MONEY_MIN: i64 = 5;
const MONEY_MAX: i64 = 20;

const EXP_PER_MESSAGE: i64 = 100;

// Extract out the code for this logic since ThreadRNG is not thread safe
fn money_rand() -> i64 {
    let mut rng = rand::thread_rng();

    rng.gen_range(MONEY_MIN..MONEY_MAX)
}
#[instrument(skip_all, fields(guild_id, message=message.content))]
async fn reward_messenger(guild_id: GuildId, ctx: &Context, message: &Message) {
    let db_helper = ContextWrapper::new_classic(ctx, guild_id);

    let user_id = message.author.id;

    // Try to get the nickname of the author
    // otherwise default to their username
    //
    // We want their nick if we need to create an entry for them
    let user_nick = message
        .author
        .nick_in(ctx, guild_id)
        .await
        .unwrap_or(message.author.name.clone());

    if let Err(e) = db_helper.create_user_if_none(user_id, &user_nick).await {
        error!("Error occuring when attempting to create new user: {:?}", e);
        // return here because we can't do the other operations without a user in the DB
        return;
    }

    if let Err(e) = db_helper
        .give_user_money(message.author.id, money_rand())
        .await
    {
        error!("Error occurred during message income: {:?}", e);
    }

    let get_user_result = db_helper.get_user(message.author.id).await;

    if let Err(e) = get_user_result {
        error!("Error occured when trying to get user info: {:?}", e);
        return;
    }

    // Panic:
    // This would only panic if we did not have a user in the DB
    // since we return early if there is an error with Mongo
    let user_info = get_user_result
        .expect("Error should've been handled already")
        .expect("User should have been created already");

    let actual_level = user_info.level;

    // Give the user exp
    match db_helper
        .give_user_exp(message.author.id, EXP_PER_MESSAGE)
        .await
    {
        Ok(res) => {
            // User has leveled up!
            if let Some(new_level) = res {
                debug!(
                    "User {}'s level has changed from {} to {}!",
                    user_info.name, actual_level, new_level
                );

                if let Err(e) = message
                    .reply_mention(
                        &ctx,
                        format!("You leveled up from {} to {}!", actual_level, new_level),
                    )
                    .await
                {
                    error!(
                        "Error when trying to send message to {}: {:?}",
                        user_info.name, e
                    );
                }
            }
        }
        Err(e) => {
            error!("Error when attempting to give user exp: {:?}", e);
        }
    }
}

#[instrument(skip_all, fields(message = message.content))]
async fn dad_bot_response(ctx: &Context, message: &Message) -> Result<Option<Message>> {
    let message_content = message.content_safe(ctx).to_lowercase();

    // To Future Me: Just plug this RegEx in on some website if you forget what it does
    // shouldn't be too hard to remember
    let re = Regex::new(r"(^|\b)(?P<im>[iI]['‘’]?m)(?P<message>.*[.,!?])?")
        .expect("Invalid regex pattern");

    let caps = re.captures(&message_content);

    if let Some(mat) = caps {
        let im_match = mat.name("im").unwrap();

        #[allow(unused_assignments)]
        let mut reply = "";

        // If there is punctuation
        if let Some(stop_match) = mat.name("message") {
            reply = stop_match.as_str().trim();

            // Trim off . or ,
            reply = &reply[0..reply.len() - 1];
        } else {
            (_, reply) = message_content.split_at(im_match.end());
            reply = reply.trim();
        }

        return message
            .reply(ctx, format!("Hi {}, I'm Johnson!", reply))
            .await
            .map_or_else(Err, |s| Ok(Some(s)));
    }

    Ok(None)
}

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip_all)]
    async fn ready(&self, _ctx: Context, _ready: Ready) {
        info!("Johnson is running!");
    }

    #[instrument(skip_all)]
    async fn message(&self, ctx: Context, message: Message) {
        if let Some(guild_id) = message.guild_id {
            // Ignore bot messages
            if message.author.bot {
                return;
            }

            if slurs::contains_slur(&message.content) {
                return;
            }

            let read_lock = ctx.data.read().await;
            let kw_responses = &read_lock.get::<SerenityCtxData>().unwrap().kwr;
            debug!("{:?}", kw_responses);

            reward_messenger(guild_id, &ctx, &message).await;
            let result = dad_bot_response(&ctx, &message).await;

            // Handle result of dad_bot_response
            match result {
                Err(e) => {
                    error!(
                        "Johnson bot failed when attempting to respond to I'm message: {:?}",
                        e
                    );
                }
                Ok(mes_opt) => {
                    if let Some(mes) = mes_opt {
                        info!("Johnson bot replied to I'm message with: {}", mes.content);
                    }
                }
            }
        }
    }
}
