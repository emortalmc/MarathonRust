mod game;
mod parkourtag;

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::str::FromStr;

use std::thread;

use redis::{Commands, Connection, PubSubCommands};
use valence::prelude::*;
use valence_anvil::AnvilWorld;
use crate::game::{Game, GameState};
use crate::parkourtag::PTGame;

pub fn main() -> ShutdownResult {
    tracing_subscriber::fmt().init();

    // redis();

    let port = std::env::var("PORT").unwrap_or(String::from("25565"));
    print!("Starting on 0.0.0.0:{}\n", port);

    valence::start_server(
        PTGameConfig {},
        ServerState {
            player_list: None,
            // games: vec![],
        })
}

pub fn redis() {
    let redis_address = std::env::var("REDIS_ADDRESS").unwrap();

    let client =
        redis::Client::open(format!("redis://{}", redis_address)).expect("Connect to client");
    let mut con = client.get_connection().expect("Get connection");

    // redis listener thread
    thread::spawn(move || {
        let mut conn: Connection = client.get_connection()?;

        conn.subscribe("proxyhello", |msg| {
            let payload: String = msg.get_payload().unwrap();

            match payload.as_ref() {
                "10" => redis::ControlFlow::Break(()),
                _ => {
                    let mut connn = client.get_connection().unwrap();
                    let _: () = connn
                        .publish("registergame", "marathonrust marathonrust 25572")
                        .expect("Failed to register game");

                    println!("Received proxyhello, re-registered game!");

                    redis::ControlFlow::Continue
                }
            }
        })
    });

    let _: () = con
        .publish("registergame", "marathonrust marathonrust 25572")
        .expect("Failed to register game");
}

pub struct PTGameConfig {}

pub struct ServerState {
    player_list: Option<PlayerListId>,
}

#[derive(Default)]
pub struct ClientState {
    id: EntityId,
}

pub struct WorldState {
    anvil: AnvilWorld,
    game_state: GameState,
    game: Box<dyn Game<PTGameConfig>>
}

const START_POS: BlockPos = BlockPos::new(0, 65, 0);

#[async_trait]
impl Config for PTGameConfig {
    type ServerState = ServerState;
    type ClientState = ClientState;
    type EntityState = ();
    type WorldState = WorldState;
    type ChunkState = ();
    type PlayerListState = ();
    type InventoryState = ();

    // fn compression_threshold(&self) -> Option<u32> {
    //     None
    // }

    fn address(&self) -> SocketAddr {
        let port = std::env::var("PORT").unwrap_or(String::from("25565"));
        SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), u16::from_str(&*port).unwrap()).into()
    }

    fn connection_mode(&self) -> ConnectionMode {
        let secret = std::env::var("VELOCITY_SECRET");
        if let Ok(secret) = secret {
            println!("Using velocity");
            ConnectionMode::Velocity { secret }
        } else {
            println!("Using online mode (if you want to use velocity, set the VELOCITY_SECRET environment variable)");
            ConnectionMode::Online
        }
    }

    async fn server_list_ping(
        &self,
        _server: &SharedServer<Self>,
        _remote_addr: SocketAddr,
        _protocol_version: i32,
    ) -> ServerListPing {
        ServerListPing::Respond {
            online_players: 0,
            max_players: 0,
            player_sample: Default::default(),
            description: "".into_text(),
            favicon_png: None,
        }
    }

    fn init(&self, server: &mut Server<Self>) {

    }

    fn update(&self, server: &mut Server<Self>) {
        server.clients.retain(|client_id, client| {
            if client.created_this_tick() {
                match server
                    .entities
                    .insert_with_uuid(EntityKind::Player, client.uuid(), ())
                {
                    Some((id, _)) => client.state.id = id,
                    None => {
                        client.disconnect("Conflicting UUID");
                        return false;
                    }
                }

                let found_game = find_available_game(&server.worlds);
                let (new_id, new_world) = match found_game {
                    Some((id, mut world)) => {

                        if let GameState::Lobby { countdown, waiting_players, .. } = world.state.game_state {
                            waiting_players.push(client_id);
                        }
                        (id, world)
                    }
                    None => {
                        create_pt_game(&mut server.worlds)
                    }
                };

                client.respawn(new_id);
                client.set_flat(true);
                client.set_game_mode(GameMode::Adventure);
                client.teleport(
                    [
                        START_POS.x as f64 + 0.5,
                        START_POS.y as f64,
                        START_POS.z as f64 + 0.5,
                    ],
                    0.0,
                    0.0,
                );
                client.set_player_list(server.state.player_list.clone());

                if let Some(id) = &server.state.player_list {
                    server.player_lists.get_mut(id).insert(
                        client.uuid(),
                        client.username(),
                        client.textures().cloned(),
                        client.game_mode(),
                        0,
                        None,
                    );
                }
            }

            if client.is_disconnected() {
                if let Some(id) = &server.state.player_list {
                    server.player_lists.get_mut(id).remove(client.uuid());
                }
                server.entities.delete(client.id);

                return false;
            }

            while let Some(event) = client.next_event() {
                if let Some(entity) = server.entities.get_mut(client.id) {
                    event.handle_default(client, entity);


                }
            }

            true
        });
    }
}

fn find_available_game(worlds: &Worlds<PTGameConfig>) -> Option<(WorldId, &World<PTGameConfig>)> {
    for (id, world) in worlds.iter() {
        let game_state = &world.state.game_state;
        let game = &world.state.game;

        // Find first game that is available
        if let GameState::Lobby { countdown, waiting_players, .. } = game_state {
            if waiting_players.len() < game.max_players() {
                return Option::from((id, world));
            }
        }
    }

    None
}

fn create_pt_game(worlds: &mut Worlds<PTGameConfig>) -> (WorldId, &World<PTGameConfig>) {
    let (world_id, world) = worlds.insert(
        DimensionId::default(),
        WorldState {
            anvil: AnvilWorld::new(PathBuf::from("./lobby")),
            game_state: GameState::Lobby {
                countdown: 20,
                waiting_players: vec![],
            },
            game: Box::new(PTGame {}),
        }

    );

    for pos in ChunkPos::at(0f64, 0f64).in_view(3) {
        match world.state.anvil.read_chunk(pos.x, pos.z) {
            Ok(Some(anvil_chunk)) => {
                let mut chunk = UnloadedChunk::new(24);

                if let Err(e) =
                    valence_anvil::to_valence(&anvil_chunk.data, &mut chunk, 4, |_| {
                        BiomeId::default()
                    })
                {
                    eprintln!("Failed to convert chunk at ({}, {}): {e}", pos.x, pos.z);
                }

                world.chunks.insert(pos, chunk, ());
            }
            Ok(None) => {
                // No chunk at this position.
                // world.chunks.insert(pos, UnloadedChunk::default(), ());
            }
            Err(e) => {
                eprintln!("Failed to read chunk at ({}, {}): {e}", pos.x, pos.z)
            }
        }
    }

    (world_id, world)
}