//! Dedicated-server Anti-Xray policies for Horizon's outbound terrain views.
mod config;
mod rules;

use std::{
    collections::{HashMap, hash_map::RandomState},
    hash::{BuildHasher, Hasher},
    sync::Mutex,
};

use mod_api::{
    Flow, Hooks, PlayerLoggedInEvent, PlayerLoggedOutEvent, PlayerLoginEvent, ServerHost,
    ServerStartedEvent, ServerTickEvent, ServerTickPhase, Text,
};

use config::{Config, DIMENSIONS};
use rules::Cover;

#[derive(Clone, Copy)]
struct Selection {
    dimension: &'static str,
    id: u32,
    bypass: bool,
}

#[derive(Default)]
struct Views {
    selections: Vec<Selection>,
    // Only network players enter this map. An entry records the last applied bypass decision.
    bypassed: HashMap<u128, bool>,
}

impl Views {
    fn configure(host: &mut dyn ServerHost) -> Result<Self, String> {
        let config = Config::load(host)?;
        let mut result = Self::default();
        let mut defined = Vec::new();
        let mut cover = None;
        let mut seed = None;
        for dimension in DIMENSIONS {
            let policy = config.policy(dimension)?;
            if !policy.enabled {
                continue;
            }
            let cover = match &cover {
                Some(cover) => cover,
                None => cover.insert(Cover::read(host)?),
            };
            let pattern_seed = if policy.mode == 1 {
                0
            } else {
                *seed.get_or_insert_with(|| RandomState::new().build_hasher().finish())
            };
            let rules = rules::compile(host, dimension, &policy, cover, pattern_seed)?;
            if rules.bands.is_empty() {
                continue;
            }
            let id = if let Some(index) = defined.iter().position(|view| view == &rules) {
                index as u32 + 1
            } else {
                let id = defined.len() as u32 + 1;
                host.server_define_terrain_view(id, &rules)
                    .map_err(|error| format!("{dimension}: terrain view rejected: {error:?}"))?;
                defined.push(rules);
                id
            };
            result.selections.push(Selection {
                dimension,
                id,
                bypass: policy.bypass,
            });
        }
        Ok(result)
    }

    fn refresh(
        selections: &[Selection],
        host: &mut dyn ServerHost,
        uuid: u128,
        previous: &mut bool,
    ) {
        let player = host.player(uuid);
        let bypass = player
            .and_then(|player| player.permission_level(host))
            .is_some_and(|level| level >= 2);
        if bypass == *previous {
            return;
        }
        for selection in selections.iter().filter(|selection| selection.bypass) {
            let view = (!bypass).then_some(selection.id);
            if let Err(error) = host.player_set_terrain_view(uuid, selection.dimension, view) {
                eprintln!(
                    "Anti-Xray {}: cannot change terrain view: {error:?}",
                    selection.dimension
                );
                if let Some(player) = player {
                    let _ =
                        player.disconnect(host, Text::translate("disconnect.disconnected", vec![]));
                }
                return;
            }
        }
        *previous = bypass;
    }
}

#[derive(Default)]
struct AntiXray {
    views: Mutex<Option<Views>>,
}

#[mod_api::horizon_mod]
impl Hooks for AntiXray {
    fn server_started(&self, host: &mut dyn ServerHost, event: &mut ServerStartedEvent) {
        let views = if event.dedicated {
            Views::configure(host)
                .map_err(|error| {
                    eprintln!(
                        "Anti-Xray rejected config/anti_xray.toml ({error}). Logins refused."
                    );
                })
                .ok()
        } else {
            Some(Views::default())
        };
        *self.views.lock().unwrap() = views;
    }

    fn player_login(&self, host: &mut dyn ServerHost, event: &mut PlayerLoginEvent<'_>) -> Flow {
        let mut guard = self.views.lock().unwrap();
        let Some(views) = guard.as_mut() else {
            return Flow::Cancel;
        };
        for selection in &views.selections {
            if let Err(error) =
                host.player_set_terrain_view(event.player, selection.dimension, Some(selection.id))
            {
                eprintln!(
                    "Anti-Xray {}: cannot select terrain view: {error:?}",
                    selection.dimension
                );
                return Flow::Cancel;
            }
        }
        Flow::Continue
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        if let Some(views) = self.views.lock().unwrap().as_mut()
            && views.selections.iter().any(|selection| selection.bypass)
        {
            let mut previous = false;
            Views::refresh(&views.selections, host, event.player, &mut previous);
            views.bypassed.insert(event.player, previous);
        }
    }

    fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        if let Some(views) = self.views.lock().unwrap().as_mut() {
            views.bypassed.remove(&event.player);
        }
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if !matches!(event.phase, ServerTickPhase::ServerTickStart { .. }) {
            return;
        }
        if let Some(views) = self.views.lock().unwrap().as_mut()
            && !views.bypassed.is_empty()
        {
            for (&uuid, previous) in &mut views.bypassed {
                Views::refresh(&views.selections, host, uuid, previous);
            }
        }
    }
}
