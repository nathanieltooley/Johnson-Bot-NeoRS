use poise::serenity_prelude::{
    self, Context, CreateMessage, FullEvent, GuildId, Mentionable, Message,
};
use poise::{FrameworkContext, FrameworkError};

use rand::distributions::{Distribution, WeightedIndex};
use rand::Rng;
use regex::Regex;
use tracing::{debug, error, info, instrument};

use crate::checks::slurs;
use crate::custom_types::command::{Data, Error, KeywordResponse};
use crate::db::{self, Database};

#[derive(Debug)]
pub struct Handler;

const MONEY_MIN: i64 = 5;
const MONEY_MAX: i64 = 20;

const EXP_PER_MESSAGE: i64 = 100;

pub async fn error_handle(error: FrameworkError<'_, Data, Error>) {
    match error {
        FrameworkError::Command { error, ctx, .. } => {
            error!(
                "An error occurred during the execution of a command, {:?}. Error: {}",
                ctx.command(),
                error
            );

            ctx.say(format!("An error has occurred: {error}"))
                .await
                .unwrap();
        }
        _ => {
            error!("Oh dear, we have an error {:?}", error)
        }
    }
}

// Extract out the code for this logic since ThreadRNG is not thread safe
fn money_rand() -> i64 {
    let mut rng = rand::thread_rng();

    rng.gen_range(MONEY_MIN..MONEY_MAX)
}

fn single_keyword_regex(kw: &str) -> Regex {
    Regex::new(&format!(r"(^|\b)({kw})($|\>)")).unwrap()
}

fn multi_keyword_regex(kws: &[String]) -> Regex {
    let mut alternate_string = String::new();

    for i in 0..kws.len() {
        // Don't put the | symbol on last keyword
        if i == kws.len() - 1 {
            alternate_string.push_str(&kws[i]);
            continue;
        }

        alternate_string.push_str(&format!("{}|", kws[i]))
    }

    Regex::new(&format!(r"(^|\b)({alternate_string})($|\>)")).unwrap()
}

fn random_choice_unweighted(responses: &[String]) -> &String {
    let rand_index = rand::thread_rng().gen_range(0..responses.len());

    &responses[rand_index]
}

fn random_choice_weighted<'a>(responses: &'a [String], weights: &Vec<f32>) -> &'a String {
    // Only errors if len of weights is 0
    let weighted_dist = WeightedIndex::new(weights).unwrap();

    &responses[weighted_dist.sample(&mut rand::thread_rng())]
}

#[instrument(skip_all, fields(guild_id, message=message.content))]
async fn reward_messenger(guild_id: GuildId, ctx: &Context, message: &Message) {
    let db_helper = Database::new((ctx, guild_id));

    // Try to get the nickname of the author
    // otherwise default to their username
    //
    // We want their nick if we need to create an entry for them
    let user_nick = message
        .author
        .nick_in(ctx, guild_id)
        .await
        .unwrap_or(message.author.name.clone());

    // we're fine to do this before the give_user_money call later because we won't use
    // this older money value
    let db_user = db_helper.get_user(&message.author).await;
    if let Err(e) = db_user {
        error!("Error occuring when attempting to get user: {:?}", e);
        // return here because we can't do the other operations without a user in the DB
        return;
    }

    let db_user = db_user.unwrap();

    if let Err(e) = db_helper
        .give_user_money(&message.author, money_rand())
        .await
    {
        error!("Error occurred during message income: {:?}", e);
    }

    let prev_level = db::exp_to_level(db_user.exp);

    // Give the user exp
    match db_helper
        .give_user_exp(&message.author, EXP_PER_MESSAGE)
        .await
    {
        Ok(res) => {
            let new_level = db::exp_to_level(res);

            if new_level > prev_level {
                debug!(
                    "User {}'s level has changed from {} to {}!",
                    message.author.mention(),
                    prev_level,
                    new_level
                );

                if let Err(e) = message
                    .reply_mention(
                        &ctx,
                        format!("You leveled up from {prev_level} to {new_level}!"),
                    )
                    .await
                {
                    error!(
                        "Error when trying to send message to {}: {:?}",
                        user_nick, e
                    );
                }
            }

            // User has leveled up!
        }
        Err(e) => {
            error!("Error when attempting to give user exp: {:?}", e);
        }
    }
}

#[instrument(skip_all, fields(message = message.content))]
async fn dad_bot_response(ctx: &Context, message: &Message) {
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
            let (_, s) = message_content.split_at(im_match.end());
            reply = s.trim();
        }

        match message
            .reply(ctx, format!("Hi {reply}, I'm Johnson!"))
            .await
        {
            Err(e) => {
                error!(
                    "Johnson bot failed when attempting to respond to I'm message: {:?}",
                    e
                );
            }
            Ok(m) => {
                info!("Johnson bot replied to im message with {}", m.content);
            }
        }
    }
}

#[instrument(skip_all)]
async fn keyword_response(ctx: &Context, message: &Message, kwrs: &[KeywordResponse]) {
    for kwr in kwrs {
        match kwr {
            KeywordResponse::SingleKW { kw, response } => {
                // no way to avoid recompiling the regex for every keyword
                let kw_re = single_keyword_regex(kw);
                // let pos_isolated_word = message.content_safe(ctx).find(&format!(" {} ", kwr.kw));
                // let pos_final_word = message.content_safe(ctx).find(&format!(" {}", kwr.kw));
                //

                // if pos_isolated_word.is_some() || pos_final_word.is_some() {
                if kw_re.is_match(&message.content_safe(ctx)) {
                    match message.reply(ctx, response).await {
                        Ok(m) => {
                            info!("Johnson Bot replied to keyword {}, with {}", kw, m.content)
                        }
                        Err(e) => {
                            error!(
                                "Johnson Bot could not reply to keyword, {}. Error: {:?}",
                                kw, e
                            );
                        }
                    }
                }
            }
            KeywordResponse::MultiKW { kws, response } => {
                let kw_re = multi_keyword_regex(kws);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    match message.reply(ctx, response).await {
                        Ok(m) => {
                            info!(
                                "Johnson Bot replied to multi keyword {:?}, with {}",
                                kws, m.content
                            )
                        }
                        Err(e) => {
                            error!(
                                "Johnson Bot could not reply to multi keyword, {:?}. Error: {:?}",
                                kws, e
                            );
                        }
                    }
                }
            }
            KeywordResponse::MultiResponse { kw, responses } => {
                let kw_re = single_keyword_regex(kw);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    match message
                        .reply(ctx, random_choice_unweighted(responses))
                        .await
                    {
                        Ok(m) => {
                            info!(
                                "Johnson Bot replied to multi response keyword {}, with {}",
                                kw, m.content
                            );
                        }
                        Err(e) => {
                            error!("Johnson Bot could not reply to multi response keyword {}. Error {:?}", kw, e);
                        }
                    }
                }
            }
            KeywordResponse::WeightedResponses {
                kw,
                responses,
                weights,
            } => {
                let kw_re = single_keyword_regex(kw);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    match message
                        .reply(ctx, random_choice_weighted(responses, weights))
                        .await
                    {
                        Ok(m) => {
                            info!(
                                "Johnson Bot replied to weighted response keyword {}, with {}",
                                kw, m.content
                            );
                        }
                        Err(e) => {
                            error!("Johnson Bot could not reply to weighted response keyword {}. Error: {:?}", kw, e);
                        }
                    }
                }
            }
            KeywordResponse::MultiKWResponse { kws, responses } => {
                let kw_re = multi_keyword_regex(kws);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    match message
                        .reply(ctx, random_choice_unweighted(responses))
                        .await
                    {
                        Ok(m) => {
                            info!(
                                "Johnson Bot replied to multi response keywords: {:?}, with {}",
                                kws, m.content
                            );
                        }
                        Err(e) => {
                            error!("Johnson Bot could not reply to multi response keywords, {:?}. Error: {:?}", kws, e);
                        }
                    }
                }
            }
            KeywordResponse::MultiKWWeightedResponses {
                kws,
                responses,
                weights,
            } => {
                let kw_re = multi_keyword_regex(kws);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    match message
                        .reply(ctx, random_choice_weighted(responses, weights))
                        .await
                    {
                        Ok(m) => {
                            info!(
                                "Johnson Bot replied to weighted response keywords: {:?}, with {}",
                                kws, m.content
                            );
                        }
                        Err(e) => {
                            error!("Johnson Bot could not reply to weighted response keywords, {:?}. Error: {:?}", kws, e);
                        }
                    }
                }
            }
        }
    }
}

#[instrument(skip_all)]
pub async fn event_handler(
    ctx: &serenity_prelude::Context,
    event: &serenity_prelude::FullEvent,
    _framework: FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        FullEvent::Ready { data_about_bot: _ } => {
            info!("Johnson is running!");
            Ok(())
        }
        FullEvent::Message { new_message } => {
            if let Some(guild_id) = new_message.guild_id {
                // Ignore bot messages
                if new_message.author.bot {
                    return Ok(());
                }

                if slurs::contains_slur(&new_message.content) {
                    let _ = new_message.delete(&ctx).await;

                    if let Ok(channel) = new_message.channel(&ctx).await {
                        // We can unwrap here because of the guild check above
                        let builder = CreateMessage::new()
                            .content("Hey! No racism is allowed in my Discord Server!");
                        let _ = channel.id().send_message(&ctx, builder).await;
                    }

                    info!(
                    "User {} said a racial slur and their message has been removed. Message: {}",
                    new_message.author.name, new_message.content
                );

                    return Ok(());
                }

                reward_messenger(guild_id, ctx, new_message).await;

                // Handle result of dad_bot_response
                dad_bot_response(ctx, new_message).await;

                let kw_responses = &data.kwr;

                keyword_response(ctx, new_message, kw_responses).await;
            }

            Ok(())
        }
        FullEvent::GuildMemberAddition { new_member } => {
            let db_helper = Database::new((ctx, new_member.guild_id));
            let server_conf = db_helper.get_server_conf(new_member.guild_id).await;

            match server_conf {
                Ok(conf) => {
                    if let Some(role) = conf.welcome_role_id {
                        if let Err(err) = new_member.add_role(ctx.http.clone(), role as u64).await {
                            error!("{:?}", err);
                            return Err(Box::new(err));
                        } else {
                            info!(
                                "Set user role on join: {} -> {}",
                                new_member.display_name(),
                                role
                            );

                            return Ok(());
                        }
                    }

                    // Do nothing if there is no role
                    Ok(())
                }
                Err(e) => match e {
                    // do nothing if there is no row
                    sqlx::error::Error::RowNotFound => Ok(()),
                    _ => {
                        error!("Couldn't get server config: {:?}", e);
                        Err(Box::new(e))
                    }
                },
            }
        }
        _ => Ok(()),
    }
}
