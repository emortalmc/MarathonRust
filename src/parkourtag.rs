use valence::client::Client;
use valence::config::Config;
use valence::prelude::{World, WorldId};
use crate::game::Game;
use crate::PTGameConfig;

pub struct PTGame {}

impl Game<PTGameConfig> for PTGame {
    fn max_players(&self) -> usize {
        8
    }

    fn min_players(&self) -> usize {
        2
    }

    fn countdown_seconds(&self) -> i32 {
        15
    }

    fn client_join(&self, client: Client<PTGameConfig>, world: World<PTGameConfig>) {

    }

    fn client_leave(&self, client: Client<PTGameConfig>, world: World<PTGameConfig>) {

    }
}