use rand::{thread_rng, Rng};
use std::collections::HashMap;

use crate::custom_types::command::{Context, Error};

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
enum Rps {
    Rock,
    Paper,
    Scissors,
}

enum RpsResult {
    Win,
    Loss,
    Tie,
}

#[poise::command(slash_command, prefix_command)]
pub async fn rock_paper_scissors(ctx: Context<'_>) -> Result<(), Error> {
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
    let rps_array = [Rps::Rock, Rps::Paper, Rps::Scissors];

    // TODO: At some point, make it so the people involved can actually choose an option for RPS
    let author_pick = rps_array[thread_rng().gen_range(0..3)];
    let opponent_pick = rps_array[thread_rng().gen_range(0..3)];

    let result = win_table
        .get(&author_pick)
        .unwrap()
        .get(&opponent_pick)
        .unwrap();

    match result {
        RpsResult::Win => {
            // You Win!
        }
        RpsResult::Tie => {
            // No one wins :(
        }
        RpsResult::Loss => {
            // You lose :((
        }
    }

    Ok(())
}
