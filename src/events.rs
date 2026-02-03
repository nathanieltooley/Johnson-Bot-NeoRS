#![allow(clippy::derived_hash_with_manual_eq)]

use std::env;
use std::fmt::Display;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use poise::serenity_prelude::{
    self, ChannelId, Context, CreateMessage, FullEvent, GuildId, Mentionable, Message, UserId,
};
use poise::{CreateReply, FrameworkContext, FrameworkError};

use problemo::*;
use rand::Rng;
use rand::distributions::{Distribution, WeightedIndex};
use regex::Regex;
use tracing::{debug, error, info, instrument};

use crate::checks::slurs;
use crate::custom_types::command::{Data, Error, KeywordResponse, SerenityCtxData};
use crate::db::{self, Database};
use crate::utils::message;

const MONEY_MIN: i64 = 5;
const MONEY_MAX: i64 = 20;

const EXP_PER_MESSAGE: i64 = 100;

const MESSAGE_TIME: Duration = Duration::from_mins(30);

gloss_error!(NewGuildMemberError, "Error processing new guild member");
static_gloss_error!(RewardError, "Error while trying to give user rewards");
static_gloss_error!(DadBotError, "Error while trying to make funny dad joke");

attachment!(GuildIdAttachment, GuildId);

#[derive(Debug)]
struct KeywordError {
    keyword: KeywordResponse,
}

impl std::error::Error for KeywordError {}
impl Display for KeywordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.keyword {
            KeywordResponse::SingleKW { kw, response: _ } => {
                write!(f, "Error while trying to respond to keyword: {kw}")
            }
            KeywordResponse::MultiKW { kws, response: _ } => {
                write!(
                    f,
                    "Error while trying to respond to multiple keywords: {kws:?}"
                )
            }
            KeywordResponse::MultiResponse { kw, responses: _ } => {
                write!(f, "Error while trying to respond to keyword: {kw}")
            }
            KeywordResponse::MultiKWResponse { kws, responses: _ } => {
                write!(
                    f,
                    "Error while trying to respond to multiple keywords: {kws:?}"
                )
            }
            KeywordResponse::WeightedResponses {
                kw,
                responses: _,
                weights: _,
            } => {
                write!(f, "Error while trying to respond to weighted keyword: {kw}")
            }
            KeywordResponse::MultiKWWeightedResponses {
                kws,
                responses: _,
                weights: _,
            } => {
                write!(
                    f,
                    "Error while trying to respond to multiple weighted keywords: {kws:?}"
                )
            }
        }
    }
}

pub async fn error_handle(error: FrameworkError<'_, Data, Error>) {
    match error {
        FrameworkError::Command { error, ctx, .. } => {
            let error_string = create_pretty_error_string(&error);

            error!(
                "An error occurred during the execution of a command, {}. Error: {}",
                ctx.command().name,
                error_string
            );

            let error_embed =
                message::embed::base_embed().description(format!("```{error_string}```"));

            if let Err(err) = ctx
                .send(
                    CreateReply::default()
                        .content("An error has occurred!")
                        .embed(error_embed),
                )
                .await
            {
                error!("Failed to send error message! Fuck! {err}");
            }
        }
        // TODO: Add handle for event_handler errors
        FrameworkError::EventHandler {
            error,
            ctx,
            event,
            framework: _,
            ..
        } => {
            let error_string = create_pretty_error_string(&error);

            error!(
                "An error occurred during the handling of an event, {}. Error: {}",
                event.snake_case_name(),
                error_string
            );

            // Send a message to the guild's error channel if needed
            if let Some(guild_attach) = error.attachment_of_type::<GuildIdAttachment>() {
                let db = Database::new(ctx);
                if let Ok(server_config) = db.get_server_conf(guild_attach.0).await
                    && let Some(error_channel) = server_config.error_channel_id
                {
                    let channel_id = ChannelId::new(error_channel as u64);

                    if let Ok(channel) = channel_id.to_channel(ctx).await
                        && let Some(g_channel) = channel.guild()
                        && let Err(err) = g_channel
                            .send_message(
                                ctx,
                                CreateMessage::new().add_embed(
                                    message::embed::base_embed()
                                        .description(format!("```{error_string}```")),
                                ),
                            )
                            .await
                    {
                        error!("Failed to send error message to error channel: {err}")
                    }
                }
            }
        }
        _ => {
            error!("Oh dear, we have an error {}", error)
        }
    }
}

fn create_pretty_error_string(problem: &Error) -> String {
    let mut error_buf: Vec<u8> = Vec::new();
    writeln!(&mut error_buf, "Error backtrace: ").expect("Writing to vec buf should not fail");

    for cause in problem {
        writeln!(&mut error_buf, "  - Error: {}", cause.error)
            .expect("Writing to vec buf should not fail");
    }

    String::from_utf8(error_buf).expect("Error message is valid utf8")
}

#[instrument(skip_all, fields(guild_id, message=message.content))]
async fn reward_messenger(
    guild_id: GuildId,
    ctx: &Context,
    message: &Message,
) -> Result<(), Problem> {
    let db_helper = Database::new(ctx);

    // we're fine to do this before the give_user_money call later because we won't use
    // this older money value
    let db_user = db_helper
        .get_user(&message.author)
        .await
        .via(RewardError::new("Couldn't get user in database"))
        .with(GuildIdAttachment::new(guild_id))?;

    db_helper
        .give_user_money(&message.author, money_rand())
        .await
        .via(RewardError::new("Couldn't give user money for message"))
        .with(GuildIdAttachment::new(guild_id))?;

    let prev_level = db::exp_to_level(db_user.exp);

    let res = db_helper
        .give_user_exp(&message.author, EXP_PER_MESSAGE)
        .await
        .via(RewardError::new("Could not give user exp for message"))
        .with(GuildIdAttachment::new(guild_id))?;

    let new_level = db::exp_to_level(res);

    if new_level > prev_level {
        debug!(
            "User {}'s level has changed from {} to {}!",
            message.author.mention(),
            prev_level,
            new_level
        );

        message
            .reply_mention(
                &ctx,
                format!("You leveled up from {prev_level} to {new_level}!"),
            )
            .await
            .via(RewardError::new(
                "Error when trying to send level up message",
            ))
            .with(GuildIdAttachment::new(guild_id))?;
    }

    Ok(())
}

#[instrument(skip_all, fields(message = message.content))]
async fn dad_bot_response(
    guild_id: GuildId,
    ctx: &Context,
    message: &Message,
) -> Result<(), Problem> {
    let message_content = message.content_safe(ctx).to_lowercase();

    // To Future Me: Just plug this RegEx in on some website if you forget what it does
    // shouldn't be too hard to remember
    let re = Regex::new(r"(^|\b)(?P<im>[iI]['‘’]?m )(?P<message>.*[.,!?])?")
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

        let message = message
            .reply(ctx, format!("Hi {reply}, I'm Johnson!"))
            .await
            .via(DadBotError::new(
                "Johnson bot failed to respond to I'm message",
            ))
            .with(GuildIdAttachment::new(guild_id))?;

        info!("Johnson bot replied to im message with {}", message.content);
    }

    Ok(())
}

#[instrument(skip_all)]
async fn keyword_response(
    guild_id: GuildId,
    ctx: &Context,
    message: &Message,
    kwrs: &[KeywordResponse],
) -> Result<(), Problem> {
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
                    let message = message
                        .reply(ctx, response)
                        .await
                        .via(KeywordError {
                            keyword: kwr.to_owned(),
                        })
                        .with(GuildIdAttachment::new(guild_id))?;

                    info!(
                        "Johnson Bot replied to keyword {}, with {}",
                        kw, message.content
                    )
                }
            }
            KeywordResponse::MultiKW { kws, response } => {
                let kw_re = multi_keyword_regex(kws);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    let message = message
                        .reply(ctx, response)
                        .await
                        .via(KeywordError {
                            keyword: kwr.to_owned(),
                        })
                        .with(GuildIdAttachment::new(guild_id))?;

                    info!(
                        "Johnson Bot replied to multi keyword {:?}, with {}",
                        kws, message.content
                    );
                }
            }
            KeywordResponse::MultiResponse { kw, responses } => {
                let kw_re = single_keyword_regex(kw);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    let message = message
                        .reply(ctx, random_choice_unweighted(responses))
                        .await
                        .via(KeywordError {
                            keyword: kwr.to_owned(),
                        })
                        .with(GuildIdAttachment::new(guild_id))?;

                    info!(
                        "Johnson Bot replied to multi response keyword {}, with {}",
                        kw, message.content
                    );
                }
            }
            KeywordResponse::WeightedResponses {
                kw,
                responses,
                weights,
            } => {
                let kw_re = single_keyword_regex(kw);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    let message = message
                        .reply(ctx, random_choice_weighted(responses, weights))
                        .await
                        .via(KeywordError {
                            keyword: kwr.to_owned(),
                        })
                        .with(GuildIdAttachment::new(guild_id))?;

                    info!(
                        "Johnson Bot replied to weighted response keyword {}, with {}",
                        kw, message.content
                    );
                }
            }
            KeywordResponse::MultiKWResponse { kws, responses } => {
                let kw_re = multi_keyword_regex(kws);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    let message = message
                        .reply(ctx, random_choice_unweighted(responses))
                        .await
                        .via(KeywordError {
                            keyword: kwr.to_owned(),
                        })
                        .with(GuildIdAttachment::new(guild_id))?;

                    info!(
                        "Johnson Bot replied to multi response keywords: {:?}, with {}",
                        kws, message.content
                    );
                }
            }
            KeywordResponse::MultiKWWeightedResponses {
                kws,
                responses,
                weights,
            } => {
                let kw_re = multi_keyword_regex(kws);

                if kw_re.is_match(&message.content_safe(ctx)) {
                    let message = message
                        .reply(ctx, random_choice_weighted(responses, weights))
                        .await
                        .via(KeywordError {
                            keyword: kwr.to_owned(),
                        })
                        .with(GuildIdAttachment::new(guild_id))?;

                    info!(
                        "Johnson Bot replied to weighted response keywords: {:?}, with {}",
                        kws, message.content
                    );
                }
            }
        }
    }

    Ok(())
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
            let http_clone = Arc::clone(&ctx.http);
            let data_clone = Arc::clone(&ctx.data);

            // Errors will have to look ugly here because they're in a separate task
            // not covered by the error_handler
            match get_friend_id() {
                Some(friend_id) => {
                    tokio::spawn(async move {
                        match http_clone.get_user(friend_id).await {
                            Ok(friend) => {
                                let friend_name =
                                    env::var("FRIEND_NAME").unwrap_or("Buddy".to_owned());

                                loop {
                                    let data_map = data_clone.read().await;
                                    let friend_info = &data_map
                                        .get::<SerenityCtxData>()
                                        .expect("Invalid ctx data")
                                        .friend_info;

                                    if friend_info.online() {
                                        if let Err(err) = friend
                                            .direct_message(
                                                &http_clone,
                                                CreateMessage::new().content(format!(
                                                    "Hey {friend_name}! Uncross your legs and sit up straight!"
                                                )),
                                            )
                                            .await
                                        {
                                            error!("Failed to send friend message: {err}");
                                        }

                                        tokio::time::sleep(MESSAGE_TIME).await;
                                    } else {
                                        tokio::time::sleep(Duration::from_secs(10)).await;
                                    }
                                }
                            }
                            Err(err) => {
                                error!("Failed to get friend user: {err}")
                            }
                        }
                    });
                }
                None => error!("No FRIEND_ID, not sending messages"),
            }

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

                let mut problems = Problems::default();

                // These have the ? at the end but will NOT exit early with an error
                // give_ok only returns early with the FailFast version of a problems recevier
                reward_messenger(guild_id, ctx, new_message)
                    .await
                    .give_ok(&mut problems)?;

                // Handle result of dad_bot_response
                dad_bot_response(guild_id, ctx, new_message)
                    .await
                    .give_ok(&mut problems)?;

                let kw_responses = &data.kwr;
                keyword_response(guild_id, ctx, new_message, kw_responses)
                    .await
                    .give_ok(&mut problems)?;

                problems.check()?;
            }

            Ok(())
        }
        FullEvent::GuildMemberAddition { new_member } => {
            let db_helper = Database::new(ctx);
            let server_conf = db_helper.get_server_conf(new_member.guild_id).await;

            match server_conf {
                Ok(conf) => {
                    if let Some(role) = conf.welcome_role_id {
                        new_member
                            .add_role(ctx.http.clone(), role as u64)
                            .await
                            .via(NewGuildMemberError::new("failed to set new member's role"))?;

                        info!(
                            "Set user role on join: {} -> {}",
                            new_member.display_name(),
                            role
                        );
                    }

                    // Do nothing if there is no role
                    Ok(())
                }
                Err(e) => match e {
                    // do nothing if there is no row
                    sqlx::error::Error::RowNotFound => Ok(()),
                    _ => Err(
                        NewGuildMemberError::as_problem("Couldn't get server config")
                            .via(e)
                            .with(GuildIdAttachment::new(new_member.guild_id)),
                    ),
                },
            }
        }
        FullEvent::PresenceUpdate { new_data } => {
            debug!("{new_data:?}");
            if let Some(friend_id) = get_friend_id()
                && friend_id == new_data.user.id
            {
                let mut data_map = ctx.data.write().await;
                let data = data_map
                    .get_mut::<SerenityCtxData>()
                    .expect("Invalid ctx data");
                data.friend_info.status = new_data.status;
            }

            Ok(())
        }
        FullEvent::VoiceStateUpdate { old: _, new } => {
            // Considered "Online" if they join a voice channel
            if let Some(ref member) = new.member {
                let user = &member.user;
                if let Some(friend_id) = get_friend_id()
                    && friend_id == user.id
                {
                    let mut data_map = ctx.data.write().await;
                    let data = data_map
                        .get_mut::<SerenityCtxData>()
                        .expect("Invalid ctx data");
                    data.friend_info.voice_status = Some(new.to_owned());
                }
            }

            Ok(())
        }
        _ => Ok(()),
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

fn get_friend_id() -> Option<UserId> {
    let id = env::var("FRIEND_ID");
    match id {
        Ok(str_id) => match str_id.parse::<u64>() {
            Ok(id) => {
                return Some(id.into());
            }
            Err(_) => error!("Invalid FRIEND_ID"),
        },
        Err(_) => error!("Missing FRIEND_ID"),
    }

    None
}
