use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, GuildId, Message, Ready, Result};

use rand::distributions::{Distribution, WeightedIndex};
use rand::Rng;
use regex::Regex;
use tracing::{debug, error, info, instrument};

use crate::checks::slurs;
use crate::custom_types::command::{KeywordResponse, SerenityCtxData};
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

fn single_keyword_regex(kw: &str) -> Regex {
    Regex::new(&format!(r"(^|\b)({})($|\>)", kw)).unwrap()
}

fn multi_keyword_regex(kws: &Vec<String>) -> Regex {
    let mut alternate_string = String::new();

    for i in 0..kws.len() {
        // Don't put the | symbol on last keyword
        if i == kws.len() - 1 {
            alternate_string.push_str(&kws[i]);
            continue;
        }

        alternate_string.push_str(&format!("{}|", kws[i]))
    }

    Regex::new(&format!(r"(^|\b)({})($|\>)", alternate_string)).unwrap()
}

fn random_choice_unweighted(responses: &Vec<String>) -> &String {
    let rand_index = rand::thread_rng().gen_range(0..responses.len());

    &responses[rand_index]
}

fn random_choice_weighted<'a>(responses: &'a Vec<String>, weights: &Vec<f32>) -> &'a String {
    // Only errors if len of weights is 0
    let weighted_dist = WeightedIndex::new(weights).unwrap();

    &responses[weighted_dist.sample(&mut rand::thread_rng())]
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
            (_, reply) = message_content.split_at(im_match.end());
            reply = reply.trim();
        }

        match message
            .reply(ctx, format!("Hi {}, I'm Johnson!", reply))
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

            reward_messenger(guild_id, &ctx, &message).await;

            // Handle result of dad_bot_response
            dad_bot_response(&ctx, &message).await;

            let read_lock = ctx.data.read().await;
            let kw_responses = &read_lock.get::<SerenityCtxData>().unwrap().kwr;

            debug!("{:?}", kw_responses);

            keyword_response(&ctx, &message, kw_responses).await;
        }
    }
}
