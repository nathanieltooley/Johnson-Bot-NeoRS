use poise::serenity_prelude::{
    self, ComponentInteractionDataKind, Content, CreateButton, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, Mentionable, Message, UserId,
};
use poise::CreateReply;
use serenity_prelude::futures::StreamExt;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, instrument};

use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;
use crate::mongo;
use crate::utils::message::interaction::wait_for_user_interaction;
use crate::utils::message::{simple_channel_message, simple_message};

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
enum Rps {
    Rock,
    Paper,
    Scissors,
}

impl FromStr for Rps {
    type Err = ();

    fn from_str(input: &str) -> Result<Rps, Self::Err> {
        match input {
            "Rock" | "rock" => Ok(Rps::Rock),
            "Paper" | "paper" => Ok(Rps::Paper),
            "Scissors" | "scissors" => Ok(Rps::Scissors),
            _ => Err(()),
        }
    }
}

/// The Result of RPS from the perspective of the command invoker
enum RpsResult {
    Win,
    Loss,
    Tie,
}

async fn get_participant_choice(
    ctx: &Context<'_>,
    rps_m: &Message,
    comp_id: UserId,
) -> Option<Rps> {
    info!("Waiting for RPS interaction");
    let mut rps_stream = rps_m
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(60))
        .stream();

    while let Some(interaction) = rps_stream.next().await {
        let user_id = interaction.user.id;

        if user_id != comp_id {
            info!("{user_id} attempted interaction with RPS choice message");
            let _ = interaction
                .create_response(
                    ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("This is not meant for you!")
                            .ephemeral(true),
                    ),
                )
                .await;
        } else {
            match interaction.data.kind {
                ComponentInteractionDataKind::Button => {
                    info!("Got RPS choice interaction!");
                    // Store in var just in case a problem comes from deleting the message before
                    // hand
                    let temp = Rps::from_str(&interaction.data.custom_id).ok();
                    // Delete the message
                    let _ = rps_m.delete(ctx).await;
                    return temp;
                }
                _ => {
                    let _ = rps_m.delete(ctx).await;
                    panic!("Unexpected item in bagging area")
                }
            };
        }
    }

    None
}

#[instrument(skip_all)]
#[poise::command(slash_command, on_error = "error_handle")]
pub async fn rock_paper_scissors(
    ctx: Context<'_>,
    #[description = "Who you're challenging"] opponent: serenity_prelude::User,
    #[description = "What you're willing to wager"] wager: u64,
) -> Result<(), Error> {
    if ctx.guild_id().is_none() {
        return Ok(());
    }

    if opponent.bot {
        return Ok(());
    }

    debug!("Getting command interaction");
    let cmd_interaction = match ctx {
        Context::Application(a) => a.interaction,
        _ => panic!("impossible"),
    };

    let mongo_driver = mongo::ContextWrapper::new_slash(ctx);
    let guild_id = ctx.guild_id().unwrap();
    let author = ctx.author();
    let author_nick = author
        .nick_in(ctx, guild_id)
        .await
        .unwrap_or(author.name.clone());

    let opponent_nick = opponent
        .nick_in(ctx, guild_id)
        .await
        .unwrap_or(opponent.name.clone());

    let rps_message_comp_auth = CreateMessage::new()
        .content(format!("Choose a Move {}!", author_nick))
        .button(CreateButton::new("rock").label("Rock"))
        .button(CreateButton::new("paper").label("Paper"))
        .button(CreateButton::new("scissors").label("Scissors"));

    let rps_message_comp_op = CreateMessage::new()
        .content(format!("Choose a Move {}!", opponent_nick))
        .button(CreateButton::new("rock").label("Rock"))
        .button(CreateButton::new("paper").label("Paper"))
        .button(CreateButton::new("scissors").label("Scissors"));

    let win_table: HashMap<Rps, HashMap<Rps, RpsResult>> = HashMap::from([
        (
            Rps::Rock,
            HashMap::from([
                (Rps::Rock, RpsResult::Tie),
                (Rps::Paper, RpsResult::Loss),
                (Rps::Scissors, RpsResult::Win),
            ]),
        ),
        (
            Rps::Paper,
            HashMap::from([
                (Rps::Rock, RpsResult::Win),
                (Rps::Paper, RpsResult::Tie),
                (Rps::Scissors, RpsResult::Loss),
            ]),
        ),
        (
            Rps::Scissors,
            HashMap::from([
                (Rps::Rock, RpsResult::Loss),
                (Rps::Paper, RpsResult::Win),
                (Rps::Scissors, RpsResult::Tie),
            ]),
        ),
    ]);

    let author_mongo_user = mongo_driver.get_user_safe(&author).await?;
    let opponent_mongo_user = mongo_driver.get_user_safe(&opponent).await?;

    debug!("Checking for author money");
    if TryInto::<u64>::try_into(author_mongo_user.vbucks).unwrap() < wager {
        cmd_interaction
            .create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("You do not have enough money for this wager!"))
                        .ephemeral(true),
                ),
            )
            .await?;

        return Ok(());
    }

    debug!("Checking for opponent money");
    if TryInto::<u64>::try_into(opponent_mongo_user.vbucks).unwrap() < wager {
        cmd_interaction
            .create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!(
                            "{opponent} does not have enough money for this wager!"
                        ))
                        .ephemeral(true),
                ),
            )
            .await?;

        return Ok(());
    }

    ctx.defer_ephemeral().await?;

    debug!("Sending acceptance message");
    let accept_message = ctx
        .channel_id()
        .send_message(
            ctx,
            CreateMessage::new()
                .content(format!("{}, {author_nick} challenges you to a Rock Paper Scissors Duel, putting ${wager} on the line. Do you accept?", opponent.mention()))
                .button(CreateButton::new("accept").label("Accept"))
                .button(CreateButton::new("decline").label("Decline")),
        )
        .await?;

    info!("Waiting for interaction from acceptance message");
    let accept_interaction =
        wait_for_user_interaction(&ctx, &accept_message, opponent.id, Duration::from_secs(60))
            .await;

    match accept_interaction {
        Some(int) => match int.data.kind {
            ComponentInteractionDataKind::Button => {
                if int.data.custom_id.as_str() == "decline" {
                    info!("{opponent} declined RPS challenge");
                    ctx.channel_id()
                        .send_message(
                            ctx,
                            CreateMessage::new().content(format!(
                                "{opponent_nick} has declined the invitation {}!",
                                author.mention()
                            )),
                        )
                        .await?;

                    let _ = accept_message.delete(ctx).await;

                    return Ok(());
                } else {
                    info!("{opponent} accepted RPS challenge");
                    ctx.channel_id()
                        .send_message(
                            ctx,
                            CreateMessage::new().content(format!(
                                "{opponent_nick} has accepted the invitation {}!",
                                author.mention()
                            )),
                        )
                        .await?;
                }
            }
            _ => {
                panic!("Should be impossible");
            }
        },
        None => {
            ctx.channel_id()
                .send_message(
                    ctx,
                    CreateMessage::new().content(format!(
                        "{opponent_nick} did not reply in time, {}!",
                        author.mention()
                    )),
                )
                .await?;
            return Ok(());
        }
    }

    debug!("Deleting acceptance message");
    let _ = accept_message.delete(ctx).await;

    info!("Sending RPS choice message for author");
    let rps_m = ctx
        .channel_id()
        .send_message(&ctx, rps_message_comp_auth)
        .await?;

    let author_choice = match get_participant_choice(&ctx, &rps_m, author.id).await {
        Some(c) => c,
        None => {
            ctx.reply("Timeout!").await?;
            return Ok(());
        }
    };

    info!("Sending RPS choice message for opponent");
    let rps_m = ctx
        .channel_id()
        .send_message(&ctx, rps_message_comp_op)
        .await?;

    let opponent_choice = match get_participant_choice(&ctx, &rps_m, opponent.id).await {
        Some(c) => c,
        None => {
            ctx.reply("Timeout!").await?;
            return Ok(());
        }
    };

    info!(
        "Author Chose: {:?}, Opponent Chose: {:?}",
        author_choice, opponent_choice
    );

    let result = win_table
        .get(&author_choice)
        .unwrap()
        .get(&opponent_choice)
        .unwrap();

    simple_channel_message(
        &ctx,
        format!(
            "{} chose {:?}, while {} chose {:?}!",
            author_nick, author_choice, opponent_nick, opponent_choice
        )
        .as_str(),
    )
    .await?;

    match result {
        RpsResult::Win => {
            simple_channel_message(&ctx, format!("{} Wins!", ctx.author().mention()).as_str())
                .await?;
            // You Win!
            debug!("Attempting to give winnings to author");
            mongo_driver
                .player_transaction(&opponent, &author, wager)
                .await?;
        }
        RpsResult::Tie => {
            simple_channel_message(
                &ctx,
                format!(
                    "{} and {} Tied :(",
                    ctx.author().mention(),
                    opponent.mention()
                )
                .as_str(),
            )
            .await?;
            // No one wins :(
        }
        RpsResult::Loss => {
            simple_channel_message(&ctx, format!("{} Wins! :((", opponent.mention()).as_str())
                .await?;
            debug!("Attempting to give winnings to opponent");
            mongo_driver
                .player_transaction(&author, &opponent, wager)
                .await?;
            // You lose :((
        }
    }

    Ok(())
}
