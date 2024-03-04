use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, GuildId, Message, Ready, Result};

use rand::Rng;
use tracing::{debug, error, info, instrument};

use crate::mongo::ContextWrapper;
use crate::utils::string::string_char_isspace;
pub struct Handler;

const MONEY_MIN: i64 = 5;
const MONEY_MAX: i64 = 20;

const EXP_PER_MESSAGE: i64 = 100;

static IM_VARIATIONS: [&str; 4] = ["im", "i'm", "i‘m", "i’m"];

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

    // TODO: Regexs?
    for im in IM_VARIATIONS {
        let im_index = message_content.find(im);

        if let Some(im_index) = im_index {
            debug!("Im found");
            // This shouldn't be a problem as im_index should never get high enough to cause an error
            let pre_im_index: i32 = (im_index as i32) - 1;

            // Make sure that either the im is at the beginning or is after a space
            // AND that the character right after it is a space as well
            if (pre_im_index < 0
                || string_char_isspace(&message_content, pre_im_index.try_into().unwrap()))
                && string_char_isspace(&message_content, im_index + im.len())
            {
                // Split at Im
                let split = message_content.split(im);

                // Grab the second part, after the Im
                let im_contents: &str = split.collect::<Vec<&str>>()[1];

                // Search for a period
                let period_index = im_contents.find('.').map_or_else(
                    || None,
                    |i| {
                        if i < im_index {
                            None
                        } else {
                            Some(i)
                        }
                    },
                );

                // Short circuits so no need to worry about panicing
                if period_index.is_some() && period_index.unwrap() > im_index {
                    // If there is a period, we end the message there
                    let full_message =
                        format!("Hi{}, I'm Johnson!", &im_contents[0..period_index.unwrap()]);

                    // Typing shenanigans
                    // Converting Result<Message, Error> -> Result<Option<Message>, Error>
                    return message
                        .reply(ctx, full_message)
                        .await
                        .map_or_else(Err, |s| Ok(Some(s)));
                } else {
                    // If there is no period, we just take the rest of the message and reply
                    let full_message = format!("Hi{}, I'm Johnson!", im_contents);

                    return message
                        .reply(ctx, full_message)
                        .await
                        .map_or_else(Err, |s| Ok(Some(s)));
                }
            }
        }
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
        debug!(message.content);
        if let Some(guild_id) = message.guild_id {
            // Ignore bot messages
            if message.author.bot {
                return;
            }

            reward_messenger(guild_id, &ctx, &message).await;
            dad_bot_response(&ctx, &message).await;
        }
    }
}
