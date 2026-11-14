// Port of com.ishland.vmp.common.chunkwatching.AreaPlayerChunkWatchingManager.
// Two area maps track which players watch which chunk: the player map at
// the server chunk loader's view distance plus one (upstream adds 1 for
// edge chunks, from the TACS hook), and the general map at the fixed
// GENERAL_PLAYER_AREA_MAP_DISTANCE derived from vanilla SpawnGroup. The
// entity-facing parts (ServerPlayerEntity's client view-distance signal,
// the ChunkFilter set, the TACS view distance) are server glue noted in
// the README; the bookkeeping below keeps the upstream shape over generic
// player ids, with the view distance passed in per call.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::area_map::{AreaMap, Listener};
use crate::mcutil;

/// Upstream GENERAL_PLAYER_AREA_MAP_DISTANCE: (int) ceil(max immediate
/// despawn range / 16.0) over vanilla SpawnGroup. Vanilla immediate ranges
/// (despawnRange - 32) are 96 for Monster and 64 for every other group,
/// so the constant is ceil(96 / 16.0) = 6; re-verify the vanilla table
/// against the target server surface when that lands.
pub const GENERAL_PLAYER_AREA_MAP_DISTANCE: i32 = general_player_area_map_distance(&[
    96, // Monster
    64, 64, 64, 64, 64, 64,
    64, // Creature, Ambient, Axolotls, UndergroundWaterCreature, WaterCreature, WaterAmbient, Misc
]);

pub const fn general_player_area_map_distance(immediate_despawn_ranges: &[i32]) -> i32 {
    let mut max = 0;
    let mut i = 0;
    while i < immediate_despawn_ranges.len() {
        if immediate_despawn_ranges[i] > max {
            max = immediate_despawn_ranges[i];
        }
        i += 1;
    }
    let ceil = (max as f64 / 16.0).ceil();
    ceil as i32
}

pub struct AreaPlayerChunkWatchingManager<'a, T: Eq + Hash + Clone + std::fmt::Debug + 'a> {
    player_area_map: AreaMap<'a, T>,
    general_player_area_map: AreaMap<'a, T>,
    positions: HashMap<T, i64>,
    view_distance_of: Box<dyn Fn(&T) -> i32 + 'a>,
}

impl<'a, T: Eq + Hash + Clone + std::fmt::Debug + 'a> AreaPlayerChunkWatchingManager<'a, T> {
    /// Upstream wires the outer add/remove listeners through the player
    /// map only, with sorted listener calls on; the general map runs bare.
    pub fn new(
        add_listener: Option<Listener<'a, T>>,
        remove_listener: Option<Listener<'a, T>>,
        view_distance_of: Box<dyn Fn(&T) -> i32 + 'a>,
    ) -> Self {
        let player_add = add_listener.map(|mut outer| {
            Box::new(move |player: &T, x: i32, z: i32| outer(player, x, z)) as Listener<'a, T>
        });
        let player_remove = remove_listener.map(|mut outer| {
            Box::new(move |player: &T, x: i32, z: i32| outer(player, x, z)) as Listener<'a, T>
        });
        Self {
            player_area_map: AreaMap::with_listeners(player_add, player_remove, true),
            general_player_area_map: AreaMap::new(),
            positions: HashMap::new(),
            view_distance_of,
        }
    }

    pub fn add(&mut self, player: T, pos: i64) {
        let x = mcutil::chunk_pos_packed_x(pos);
        let z = mcutil::chunk_pos_packed_z(pos);
        let view_distance = (self.view_distance_of)(&player);

        self.player_area_map
            .add(player.clone(), x, z, view_distance);
        self.general_player_area_map
            .add(player.clone(), x, z, GENERAL_PLAYER_AREA_MAP_DISTANCE);

        self.positions
            .insert(player, mcutil::get_coordinate_key(x, z));
    }

    pub fn remove(&mut self, player: &T) {
        self.player_area_map.remove(player);
        self.general_player_area_map.remove(player);

        self.positions.remove(player);
    }

    pub fn move_player(&mut self, current_pos: i64, player: &T) {
        let x = mcutil::chunk_pos_packed_x(current_pos);
        let z = mcutil::chunk_pos_packed_z(current_pos);
        let view_distance = (self.view_distance_of)(player);

        self.player_area_map.update(player, x, z, view_distance);
        self.general_player_area_map
            .update(player, x, z, GENERAL_PLAYER_AREA_MAP_DISTANCE);

        self.positions
            .insert(player.clone(), mcutil::get_coordinate_key(x, z));
    }

    /// Upstream onWatchDistanceChange: re-apply every player at their
    /// current position with the fresh view distance. The ChunkFilter
    /// reset it performs alongside is server glue.
    pub fn on_watch_distance_change(&mut self) {
        let entries: Vec<(T, i64)> = self
            .positions
            .iter()
            .map(|(player, pos)| (player.clone(), *pos))
            .collect();
        for (player, pos) in entries {
            let x = mcutil::get_coordinate_x(pos);
            let z = mcutil::get_coordinate_z(pos);
            let view_distance = (self.view_distance_of)(&player);
            self.player_area_map.update(&player, x, z, view_distance);
            self.general_player_area_map
                .update(&player, x, z, GENERAL_PLAYER_AREA_MAP_DISTANCE);
        }
    }

    /// Upstream tick() re-applies the chunk filter for players whose
    /// client view distance changed; the change signal is server glue,
    /// so the port exposes the same bookkeeping through move_player.
    pub fn get_players_watching_chunk(&self, l: i64) -> Option<&HashSet<T>> {
        self.player_area_map.get_objects_in_range(l)
    }

    pub fn get_players_watching_chunk_array(&self, coordinate_key: i64) -> Vec<T> {
        self.player_area_map
            .get_objects_in_range_array(coordinate_key)
    }

    pub fn get_players_in_general_area_map(&self, coordinate_key: i64) -> Vec<T> {
        self.general_player_area_map
            .get_objects_in_range_array(coordinate_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> AreaPlayerChunkWatchingManager<'static, u32> {
        AreaPlayerChunkWatchingManager::new(None, None, Box::new(|_| 8))
    }

    #[test]
    fn add_places_player_in_both_maps() {
        let mut m = manager();
        let pos = mcutil::get_coordinate_key(5, -3);
        m.add(1, pos);
        assert!(m.get_players_watching_chunk(pos).unwrap().contains(&1));
        // general map covers 6 around the player chunk
        assert!(
            m.get_players_in_general_area_map(mcutil::get_coordinate_key(5 + 6, -3))
                .contains(&1)
        );
        assert!(
            m.get_players_in_general_area_map(mcutil::get_coordinate_key(5 + 7, -3))
                .is_empty()
        );
    }

    #[test]
    fn move_player_updates_both_maps() {
        let mut m = manager();
        m.add(1, mcutil::get_coordinate_key(0, 0));
        m.move_player(mcutil::get_coordinate_key(10, 0), &1);
        assert!(
            m.get_players_watching_chunk(mcutil::get_coordinate_key(0, 0))
                .is_none()
        );
        assert!(
            m.get_players_watching_chunk(mcutil::get_coordinate_key(10, 0))
                .unwrap()
                .contains(&1)
        );
        assert!(
            m.get_players_in_general_area_map(mcutil::get_coordinate_key(3, 0))
                .is_empty()
        );
    }

    #[test]
    fn remove_clears_everywhere() {
        let mut m = manager();
        m.add(1, mcutil::get_coordinate_key(0, 0));
        m.remove(&1);
        assert!(
            m.get_players_watching_chunk(mcutil::get_coordinate_key(0, 0))
                .is_none()
        );
        assert!(
            m.get_players_in_general_area_map(mcutil::get_coordinate_key(0, 0))
                .is_empty()
        );
    }

    #[test]
    fn two_players_share_a_chunk_view() {
        let mut m = manager();
        m.add(1, mcutil::get_coordinate_key(0, 0));
        m.add(2, mcutil::get_coordinate_key(1, 0));
        let watching = m
            .get_players_watching_chunk(mcutil::get_coordinate_key(1, 0))
            .unwrap();
        assert!(watching.contains(&1) && watching.contains(&2));
        m.remove(&1);
        let left = m
            .get_players_watching_chunk(mcutil::get_coordinate_key(1, 0))
            .unwrap();
        assert!(left.contains(&2) && !left.contains(&1));
    }

    #[test]
    fn watch_distance_change_reapplies_positions() {
        let vd = std::rc::Rc::new(std::cell::Cell::new(8));
        let shared = vd.clone();
        let mut m: AreaPlayerChunkWatchingManager<u32> =
            AreaPlayerChunkWatchingManager::new(None, None, Box::new(move |_| shared.get()));
        m.add(1, mcutil::get_coordinate_key(0, 0));
        vd.set(2);
        m.on_watch_distance_change();
        // view distance now 2: still watching own chunk, no longer 3 away
        assert!(
            m.get_players_watching_chunk(mcutil::get_coordinate_key(0, 0))
                .unwrap()
                .contains(&1)
        );
        assert!(
            m.get_players_watching_chunk(mcutil::get_coordinate_key(3, 0))
                .is_none()
        );
    }

    #[test]
    fn general_distance_is_six() {
        assert_eq!(GENERAL_PLAYER_AREA_MAP_DISTANCE, 6);
        assert_eq!(general_player_area_map_distance(&[96, 64, 64]), 6);
        assert_eq!(general_player_area_map_distance(&[64]), 4);
    }
}
