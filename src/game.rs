use valence::client::{Client, ClientId};
use valence::config::Config;
use valence::Ticks;
use valence::world::{World, WorldId};

pub trait Game<C: Config>: Send + Sync + 'static {
    fn max_players(&self) -> usize;
    fn min_players(&self) -> usize;
    fn countdown_seconds(&self) -> i32;

    fn client_join(&self, client: Client<C>, world: World<C>);
    fn client_leave(&self, client: Client<C>, world: World<C>);
}

pub enum GameState {
    /// Waiting for players to join
    Lobby {
        countdown: Ticks,
        waiting_players: Vec<ClientId>
    },
    /// The game is playing.
    Main {
        remaining_players: Vec<ClientId>,
    }
}

