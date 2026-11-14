// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
// vmp-fabric slice 1 (port of RelativityMC/VMP-fabric, MIT, ishland;
// upstream read at ver/26.1.1, commit 7e89476).
//
// Logic-only core: the full vmp.properties config model, the chunk
// coordinate-key helpers the watching maps run on, the pooled-object
// utility, the AreaMap spatial index behind player watching, and the
// player-chunk-watching manager's bookkeeping. The server-coupled
// surfaces (Netty event loops, async chunks, portals, async logging,
// entity tracking, ticket propagator, biome access, collections, spawn
// density cap) are seams noted in the README.

pub mod area_map {
    // Port of com.ishland.vmp.common.maps.AreaMap (AreaMap.java).
    // A spatial index keyed by packed chunk coordinate: every tracked object
    // covers a (2*viewDistance+1) square of chunk cells and each cell holds
    // the set of objects watching it. Upstream keeps its per-cell sets
    // insertion-ordered identity sets drawn from a SimpleObjectPool; the
    // behavior here is the same on plain sets (the pool itself lives in
    // pool.rs). The validate() debug hook (vmp.debugAreaMap) is upstream's
    // test-only assertion and is not carried.

    use std::collections::{HashMap, HashSet};
    use std::hash::Hash;

    use crate::mcutil;

    pub type Listener<'a, T> = Box<dyn FnMut(&T, i32, i32) + 'a>;

    pub struct AreaMap<'a, T: Eq + Hash + Clone> {
        map: HashMap<i64, HashSet<T>>,
        view_distances: HashMap<T, i32>,
        last_centers: HashMap<T, i64>,
        add_listener: Option<Listener<'a, T>>,
        remove_listener: Option<Listener<'a, T>>,
        sort_listener_calls: bool,
    }

    impl<'a, T: Eq + Hash + Clone + std::fmt::Debug> Default for AreaMap<'a, T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<'a, T: Eq + Hash + Clone + std::fmt::Debug> AreaMap<'a, T> {
        pub fn new() -> Self {
            Self::with_listeners(None, None, false)
        }

        pub fn with_listeners(
            add_listener: Option<Listener<'a, T>>,
            remove_listener: Option<Listener<'a, T>>,
            sort_listener_calls: bool,
        ) -> Self {
            Self {
                map: HashMap::new(),
                view_distances: HashMap::new(),
                last_centers: HashMap::new(),
                add_listener,
                remove_listener,
                sort_listener_calls,
            }
        }

        /// Upstream getObjectsInRange: the live set for a cell, or the shared
        /// empty set when nothing watches it (an empty Option here).
        pub fn get_objects_in_range(&self, coordinate_key: i64) -> Option<&HashSet<T>> {
            self.map.get(&coordinate_key)
        }

        /// Upstream getObjectsInRangeArray: the raw set contents.
        pub fn get_objects_in_range_array(&self, coordinate_key: i64) -> Vec<T> {
            self.map
                .get(&coordinate_key)
                .map(|set| set.iter().cloned().collect())
                .unwrap_or_default()
        }

        pub fn add(&mut self, object: T, x: i32, z: i32, raw_view_distance: i32) {
            let view_distance = raw_view_distance;
            self.view_distances.insert(object.clone(), view_distance);
            self.last_centers
                .insert(object.clone(), mcutil::get_coordinate_key(x, z));

            let mut add_listener = self.add_listener.take();
            if self.sort_listener_calls && add_listener.is_some() {
                let mut keys: Vec<i64> = Vec::with_capacity(
                    ((2 * view_distance + 1) * (2 * view_distance + 1)) as usize,
                );
                for xx in (x - view_distance)..=(x + view_distance) {
                    for zz in (z - view_distance)..=(z + view_distance) {
                        self.add0(xx, zz, &object, false, &mut add_listener);
                        keys.push(mcutil::get_coordinate_key(xx, zz));
                    }
                }
                self.notify_listeners_sorted(&object, x, z, &mut add_listener, &mut keys);
            } else {
                for xx in (x - view_distance)..=(x + view_distance) {
                    for zz in (z - view_distance)..=(z + view_distance) {
                        self.add0(xx, zz, &object, true, &mut add_listener);
                    }
                }
            }
            self.add_listener = add_listener;
        }

        pub fn remove(&mut self, object: &T) {
            if !self.view_distances.contains_key(object) {
                return;
            }
            let view_distance = self.view_distances.remove(object).expect("tracked");
            let last_center = self.last_centers.remove(object).expect("tracked");
            let x = mcutil::get_coordinate_x(last_center);
            let z = mcutil::get_coordinate_z(last_center);

            let mut remove_listener = self.remove_listener.take();
            if self.sort_listener_calls && remove_listener.is_some() {
                let mut keys: Vec<i64> = Vec::with_capacity(
                    ((2 * view_distance + 1) * (2 * view_distance + 1)) as usize,
                );
                for xx in (x - view_distance)..=(x + view_distance) {
                    for zz in (z - view_distance)..=(z + view_distance) {
                        self.remove0(xx, zz, object, false, &mut remove_listener);
                        keys.push(mcutil::get_coordinate_key(xx, zz));
                    }
                }
                self.notify_listeners_sorted(object, x, z, &mut remove_listener, &mut keys);
            } else {
                for xx in (x - view_distance)..=(x + view_distance) {
                    for zz in (z - view_distance)..=(z + view_distance) {
                        self.remove0(xx, zz, object, true, &mut remove_listener);
                    }
                }
            }
            self.remove_listener = remove_listener;
        }

        pub fn update(&mut self, object: &T, x: i32, z: i32, raw_view_distance: i32) {
            if !self.view_distances.contains_key(object) {
                panic!("Tried to update {:?} when not in map", object);
            }
            let view_distance = raw_view_distance;
            let old_view_distance = self
                .view_distances
                .insert(object.clone(), view_distance)
                .expect("tracked");
            let old_center = self
                .last_centers
                .insert(object.clone(), mcutil::get_coordinate_key(x, z))
                .expect("tracked");
            let old_x = mcutil::get_coordinate_x(old_center);
            let old_z = mcutil::get_coordinate_z(old_center);

            self.update_adds(object, old_x, old_z, old_view_distance, x, z, view_distance);
            self.update_removals(object, old_x, old_z, old_view_distance, x, z, view_distance);
        }

        pub fn unique_objects(&self) -> usize {
            self.last_centers.len()
        }

        fn update_adds(
            &mut self,
            object: &T,
            old_x: i32,
            old_z: i32,
            old_view_distance: i32,
            new_x: i32,
            new_z: i32,
            new_view_distance: i32,
        ) {
            let x_lower = old_x - old_view_distance;
            let x_higher = old_x + old_view_distance;
            let z_lower = old_z - old_view_distance;
            let z_higher = old_z + old_view_distance;

            let mut add_listener = self.add_listener.take();
            if self.sort_listener_calls && add_listener.is_some() {
                let mut keys: Vec<i64> = Vec::with_capacity(
                    ((2 * new_view_distance + 1) * (2 * new_view_distance + 1)) as usize,
                );
                for xx in (new_x - new_view_distance)..=(new_x + new_view_distance) {
                    for zz in (new_z - new_view_distance)..=(new_z + new_view_distance) {
                        if !is_in_range(x_lower, x_higher, z_lower, z_higher, xx, zz) {
                            self.add0(xx, zz, object, false, &mut add_listener);
                            keys.push(mcutil::get_coordinate_key(xx, zz));
                        }
                    }
                }
                self.notify_listeners_sorted(object, new_x, new_z, &mut add_listener, &mut keys);
            } else {
                for xx in (new_x - new_view_distance)..=(new_x + new_view_distance) {
                    for zz in (new_z - new_view_distance)..=(new_z + new_view_distance) {
                        if !is_in_range(x_lower, x_higher, z_lower, z_higher, xx, zz) {
                            self.add0(xx, zz, object, true, &mut add_listener);
                        }
                    }
                }
            }
            self.add_listener = add_listener;
        }

        fn update_removals(
            &mut self,
            object: &T,
            old_x: i32,
            old_z: i32,
            old_view_distance: i32,
            new_x: i32,
            new_z: i32,
            new_view_distance: i32,
        ) {
            let x_lower = new_x - new_view_distance;
            let x_higher = new_x + new_view_distance;
            let z_lower = new_z - new_view_distance;
            let z_higher = new_z + new_view_distance;

            let mut remove_listener = self.remove_listener.take();
            if self.sort_listener_calls && remove_listener.is_some() {
                let mut keys: Vec<i64> = Vec::with_capacity(
                    ((2 * old_view_distance + 1) * (2 * old_view_distance + 1)) as usize,
                );
                for xx in (old_x - old_view_distance)..=(old_x + old_view_distance) {
                    for zz in (old_z - old_view_distance)..=(old_z + old_view_distance) {
                        if !is_in_range(x_lower, x_higher, z_lower, z_higher, xx, zz) {
                            self.remove0(xx, zz, object, false, &mut remove_listener);
                            keys.push(mcutil::get_coordinate_key(xx, zz));
                        }
                    }
                }
                self.notify_listeners_sorted(object, new_x, new_z, &mut remove_listener, &mut keys);
            } else {
                for xx in (old_x - old_view_distance)..=(old_x + old_view_distance) {
                    for zz in (old_z - old_view_distance)..=(old_z + old_view_distance) {
                        if !is_in_range(x_lower, x_higher, z_lower, z_higher, xx, zz) {
                            self.remove0(xx, zz, object, true, &mut remove_listener);
                        }
                    }
                }
            }
            self.remove_listener = remove_listener;
        }

        fn add0(
            &mut self,
            xx: i32,
            zz: i32,
            object: &T,
            notify_listeners: bool,
            add_listener: &mut Option<Listener<'a, T>>,
        ) {
            let set = self
                .map
                .entry(mcutil::get_coordinate_key(xx, zz))
                .or_default();
            set.insert(object.clone());
            if notify_listeners {
                if let Some(listener) = add_listener {
                    listener(object, xx, zz);
                }
            }
        }

        fn remove0(
            &mut self,
            xx: i32,
            zz: i32,
            object: &T,
            notify_listeners: bool,
            remove_listener: &mut Option<Listener<'a, T>>,
        ) {
            let coordinate_key = mcutil::get_coordinate_key(xx, zz);
            let set = self
                .map
                .get_mut(&coordinate_key)
                .unwrap_or_else(|| panic!("Expect non-null set in [{},{}]", xx, zz));
            if !set.remove(object) {
                panic!("Expect {:?} in set ([{},{}])", object, xx, zz);
            }
            if set.is_empty() {
                self.map.remove(&coordinate_key);
                // upstream releases the set back to its pool here
            }
            if notify_listeners {
                if let Some(listener) = remove_listener {
                    listener(object, xx, zz);
                }
            }
        }

        fn notify_listeners_sorted(
            &self,
            object: &T,
            x: i32,
            z: i32,
            listener: &mut Option<Listener<'a, T>>,
            keys: &mut [i64],
        ) {
            keys.sort_by_key(|pos| {
                chebyshev_distance(
                    x,
                    z,
                    mcutil::get_coordinate_x(*pos),
                    mcutil::get_coordinate_z(*pos),
                )
            });
            for &pos in keys.iter() {
                if let Some(l) = listener {
                    l(
                        object,
                        mcutil::get_coordinate_x(pos),
                        mcutil::get_coordinate_z(pos),
                    );
                }
            }
        }
    }

    fn is_in_range(
        x_lower: i32,
        x_higher: i32,
        z_lower: i32,
        z_higher: i32,
        x: i32,
        z: i32,
    ) -> bool {
        x >= x_lower && x <= x_higher && z >= z_lower && z <= z_higher
    }

    fn chebyshev_distance(x0: i32, z0: i32, x1: i32, z1: i32) -> i32 {
        i32::max((x0 - x1).abs(), (z0 - z1).abs())
    }

    #[cfg(test)]
    mod tests {
        use std::cell::RefCell;
        use std::rc::Rc;

        use super::*;

        fn key(x: i32, z: i32) -> i64 {
            mcutil::get_coordinate_key(x, z)
        }

        #[test]
        fn add_covers_the_view_square() {
            let mut map: AreaMap<u32> = AreaMap::new();
            map.add(7, 10, 10, 1);
            for xx in 9..=11 {
                for zz in 9..=11 {
                    assert!(map.get_objects_in_range(key(xx, zz)).unwrap().contains(&7));
                }
            }
            assert!(map.get_objects_in_range(key(8, 10)).is_none());
            assert_eq!(map.unique_objects(), 1);
        }

        #[test]
        fn remove_is_a_no_op_when_not_tracked_and_cleans_cells() {
            let mut map: AreaMap<u32> = AreaMap::new();
            map.remove(&7); // absent: no-op
            map.add(7, 0, 0, 1);
            map.remove(&7);
            assert!(map.get_objects_in_range(key(0, 0)).is_none());
            assert_eq!(map.unique_objects(), 0);
        }

        #[test]
        fn update_moves_only_the_difference() {
            let mut map: AreaMap<u32> = AreaMap::new();
            map.add(1, 0, 0, 1);
            map.update(&1, 1, 0, 1);
            // shared column stays watched
            for xx in 0..=2 {
                assert!(map.get_objects_in_range(key(xx, 0)).unwrap().contains(&1));
            }
            // lost the far column, gained the far column
            assert!(map.get_objects_in_range(key(-1, 0)).is_none());
            assert!(map.get_objects_in_range(key(2, 1)).unwrap().contains(&1));
            assert!(map.get_objects_in_range(key(0, 1)).unwrap().contains(&1));
        }

        #[test]
        fn update_growing_view_distance_fills_the_ring() {
            let mut map: AreaMap<u32> = AreaMap::new();
            map.add(1, 0, 0, 0);
            assert!(map.get_objects_in_range(key(0, 0)).unwrap().contains(&1));
            map.update(&1, 0, 0, 1);
            assert!(map.get_objects_in_range(key(1, 1)).unwrap().contains(&1));
            assert!(map.get_objects_in_range(key(-1, 0)).unwrap().contains(&1));
        }

        #[test]
        fn sorted_add_listener_walks_by_chebyshev_distance() {
            let calls: Rc<RefCell<Vec<(i32, i32)>>> = Rc::new(RefCell::new(Vec::new()));
            let sink = calls.clone();
            let mut map: AreaMap<u32> = AreaMap::with_listeners(
                Some(Box::new(move |_obj, x, z| sink.borrow_mut().push((x, z)))),
                None,
                true,
            );
            map.add(1, 0, 0, 2);
            let calls = calls.borrow();
            let distances: Vec<i32> = calls
                .iter()
                .map(|(x, z)| chebyshev_distance(0, 0, *x, *z))
                .collect();
            let mut sorted = distances.clone();
            sorted.sort();
            assert_eq!(distances, sorted);
            assert_eq!(calls.len(), 25); // 5x5
        }

        #[test]
        fn unsorted_listener_fires_per_cell_inline() {
            let calls: Rc<RefCell<Vec<(i32, i32)>>> = Rc::new(RefCell::new(Vec::new()));
            let sink = calls.clone();
            let mut map: AreaMap<u32> = AreaMap::with_listeners(
                Some(Box::new(move |_obj, x, z| sink.borrow_mut().push((x, z)))),
                None,
                false,
            );
            map.add(1, 0, 0, 1);
            assert_eq!(calls.borrow().len(), 9);
            map.remove(&1);
            // no remove listener registered: nothing extra
            assert_eq!(calls.borrow().len(), 9);
        }

        #[test]
        fn two_objects_share_cells_and_remove_independently() {
            let mut map: AreaMap<u32> = AreaMap::new();
            map.add(1, 0, 0, 1);
            map.add(2, 1, 0, 1);
            let shared = map.get_objects_in_range(key(1, 0)).unwrap();
            assert!(shared.contains(&1) && shared.contains(&2));
            map.remove(&1);
            let left = map.get_objects_in_range(key(1, 0)).unwrap();
            assert!(left.contains(&2) && !left.contains(&1));
            assert_eq!(map.unique_objects(), 1);
        }

        #[test]
        #[should_panic(expected = "when not in map")]
        fn update_of_untracked_object_panics() {
            let mut map: AreaMap<u32> = AreaMap::new();
            map.update(&9, 0, 0, 1);
        }
    }
}
pub mod chunk_watching {
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
            self.general_player_area_map.add(
                player.clone(),
                x,
                z,
                GENERAL_PLAYER_AREA_MAP_DISTANCE,
            );

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
                self.general_player_area_map.update(
                    &player,
                    x,
                    z,
                    GENERAL_PLAYER_AREA_MAP_DISTANCE,
                );
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
}
pub mod config {
    // Port of com.ishland.vmp.common.config.Config (Config.java).
    // Upstream reads vmp.properties through java.util.Properties, resolves
    // every key with its default on a missing or malformed value, writes the
    // normalized file back under the "Configuration file for VMP" comment,
    // and forces booleans that another mod lists in its
    // vmp:incompatibleConfig array to disabled (unless the
    // vmp.ignoreIncompatibleConfig system property is set). The Fabric
    // mod-container walk is the caller's glue; the forcing logic and the
    // exact warning wording are kept 1:1.

    pub const TARGET_CHUNK_SEND_RATE_DEFAULT: i32 = -1;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Config {
        pub target_chunk_send_rate: i32,
        pub use_async_logging: bool,
        pub use_optimized_entity_tracking: bool,
        pub optimized_entity_tracking_use_staging_area: bool,
        pub use_multiple_netty_event_loops: bool,
        pub use_async_portals: bool,
        pub use_async_chunks_on_login: bool,
        pub use_async_chunks_on_some_commands: bool,
        pub show_async_loading_messages: bool,
        pub show_chunk_tracking_messages: bool,
    }

    impl Default for Config {
        fn default() -> Self {
            Config {
                target_chunk_send_rate: TARGET_CHUNK_SEND_RATE_DEFAULT,
                use_async_logging: true,
                use_optimized_entity_tracking: true,
                optimized_entity_tracking_use_staging_area: true,
                use_multiple_netty_event_loops: true,
                use_async_portals: true,
                use_async_chunks_on_login: true,
                use_async_chunks_on_some_commands: false,
                show_async_loading_messages: true,
                show_chunk_tracking_messages: true,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Incompatibility {
        pub key: String,
        pub mod_id: String,
        pub mod_version: String,
    }

    pub struct LoadResult {
        pub config: Config,
        /// The normalized vmp.properties text upstream writes back, with
        /// every key resolved and the forcing NOT applied (upstream writes
        /// the parsed value before the incompatibility pass).
        pub normalized_properties: String,
        /// The warning messages in upstream order.
        pub warnings: Vec<String>,
    }

    impl Config {
        pub fn load(
            properties_text: &str,
            incompatibilities: &[Incompatibility],
            ignore_incompatible: bool,
        ) -> LoadResult {
            let entries = parse_properties(properties_text);
            let mut normalized: Vec<(String, String)> = Vec::new();
            let mut warnings: Vec<String> = Vec::new();

            let mut config = Config::default();

            config.target_chunk_send_rate = get_int(
                &entries,
                "target_chunk_send_rate",
                TARGET_CHUNK_SEND_RATE_DEFAULT,
                &mut normalized,
            );
            config.use_async_logging = get_boolean(
                &entries,
                "use_async_logging",
                true,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );
            config.use_optimized_entity_tracking = get_boolean(
                &entries,
                "use_optimized_entity_tracking",
                true,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );
            config.optimized_entity_tracking_use_staging_area = get_boolean(
                &entries,
                "optimized_entity_tracking_use_staging_area",
                true,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );
            config.use_multiple_netty_event_loops = get_boolean(
                &entries,
                "use_multiple_netty_event_loops",
                true,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );
            config.use_async_portals = get_boolean(
                &entries,
                "use_async_portals",
                true,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );
            config.use_async_chunks_on_login = get_boolean(
                &entries,
                "use_async_chunks_on_login_v2",
                true,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );
            config.use_async_chunks_on_some_commands = get_boolean(
                &entries,
                "use_async_chunks_on_some_commands",
                false,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );
            config.show_async_loading_messages = get_boolean(
                &entries,
                "show_async_loading_messages",
                true,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );
            config.show_chunk_tracking_messages = get_boolean(
                &entries,
                "show_chunk_tracking_messages",
                true,
                incompatibilities,
                ignore_incompatible,
                &mut normalized,
                &mut warnings,
            );

            let mut normalized_properties = String::from("#Configuration file for VMP\n");
            for (key, value) in &normalized {
                normalized_properties.push_str(key);
                normalized_properties.push('=');
                normalized_properties.push_str(value);
                normalized_properties.push('\n');
            }

            LoadResult {
                config,
                normalized_properties,
                warnings,
            }
        }
    }

    fn lookup<'a>(entries: &'a [(String, String)], key: &str) -> Option<&'a str> {
        // java.util.Properties: the last assignment wins.
        entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn get_int(
        entries: &[(String, String)],
        key: &str,
        def: i32,
        normalized: &mut Vec<(String, String)>,
    ) -> i32 {
        let value = match lookup(entries, key) {
            Some(text) => match text.trim().parse::<i32>() {
                Ok(i) => i,
                Err(_) => def,
            },
            None => def,
        };
        normalized.push((key.to_string(), value.to_string()));
        value
    }

    fn get_boolean0(
        entries: &[(String, String)],
        key: &str,
        def: bool,
        normalized: &mut Vec<(String, String)>,
    ) -> bool {
        let parsed = lookup(entries, key)
            .map(|text| {
                let trimmed = text.trim();
                if trimmed.eq_ignore_ascii_case("true") {
                    true
                } else if trimmed.eq_ignore_ascii_case("false") {
                    false
                } else {
                    def
                }
            })
            .unwrap_or(def);
        normalized.push((key.to_string(), parsed.to_string()));
        parsed
    }

    fn get_boolean(
        entries: &[(String, String)],
        key: &str,
        def: bool,
        incompatibilities: &[Incompatibility],
        ignore_incompatible: bool,
        normalized: &mut Vec<(String, String)>,
        warnings: &mut Vec<String>,
    ) -> bool {
        let mut value = get_boolean0(entries, key, def, normalized);
        for incompatibility in incompatibilities.iter().filter(|i| i.key == key) {
            if ignore_incompatible {
                warnings.push(format!(
                    "Ignoring incompatibility of {} (defined in {}@{})",
                    key, incompatibility.mod_id, incompatibility.mod_version
                ));
            } else {
                warnings.push(format!(
                "Forcing {} in vmp.properties to be disabled (defined in {}@{}) (you may override this with -Dvmp.ignoreIncompatibleConfig=true)",
                key, incompatibility.mod_id, incompatibility.mod_version
            ));
                value = false;
            }
        }
        value
    }

    /// The java.util.Properties reader subset the config needs: comment lines
    /// (# or !), =/:/whitespace separators, escape sequences, and backslash
    /// line continuations. Properties.store output is plain key=value lines.
    fn parse_properties(text: &str) -> Vec<(String, String)> {
        let mut entries = Vec::new();
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                continue;
            }
            // Backslash continuations: a line ends with an ODD number of
            // trailing backslashes when the last one is an escape marker.
            let mut logical = trimmed.to_string();
            while trailing_backslashes(&logical) % 2 == 1 {
                logical.pop(); // drop the continuation marker
                match lines.next() {
                    Some(next) => logical.push_str(next.trim_start()),
                    None => break,
                }
            }
            if let Some((key, value)) = split_entry(&logical) {
                entries.push((key, value));
            }
        }
        entries
    }

    fn trailing_backslashes(s: &str) -> usize {
        s.chars().rev().take_while(|c| *c == '\\').count()
    }

    fn split_entry(line: &str) -> Option<(String, String)> {
        let chars: Vec<char> = line.chars().collect();
        let mut key = String::new();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\\' {
                if let Some(next) = chars.get(i + 1) {
                    key.push(*next);
                    i += 2;
                    continue;
                }
                return None;
            }
            if c == '=' || c == ':' {
                i += 1;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                break;
            }
            if c.is_whitespace() {
                // whitespace separator: the value may start after more whitespace
                i += 1;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                if i < chars.len() && chars[i] != '=' && chars[i] != ':' {
                    return Some((key, unescape(&chars[i..].iter().collect::<String>())));
                }
                continue;
            }
            key.push(c);
            i += 1;
        }
        if i > chars.len() {
            return None;
        }
        let value = unescape(&chars[i..].iter().collect::<String>());
        Some((key, value))
    }

    fn unescape(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('f') => out.push('\u{c}'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    let code = u32::from_str_radix(&hex, 16).unwrap_or(0);
                    out.push(char::from_u32(code).unwrap_or('\u{0}'));
                }
                Some(other) => out.push(other),
                None => {}
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn inc(key: &str) -> Incompatibility {
            Incompatibility {
                key: key.to_string(),
                mod_id: "othermod".to_string(),
                mod_version: "1.2.3".to_string(),
            }
        }

        #[test]
        fn missing_file_yields_defaults() {
            let result = Config::load("", &[], false);
            assert_eq!(result.config, Config::default());
            assert_eq!(result.config.target_chunk_send_rate, -1);
            assert!(result.config.use_async_chunks_on_login);
            assert!(!result.config.use_async_chunks_on_some_commands);
            assert!(result.warnings.is_empty());
        }

        #[test]
        fn normalized_write_back_lists_every_key() {
            let result = Config::load("# manual edit\ntarget_chunk_send_rate=5\n", &[], false);
            assert!(
                result
                    .normalized_properties
                    .starts_with("#Configuration file for VMP\n")
            );
            for key in [
                "target_chunk_send_rate",
                "use_async_logging",
                "use_async_chunks_on_login_v2",
            ] {
                assert!(
                    result.normalized_properties.contains(&format!("{key}=")),
                    "{key} missing"
                );
            }
        }

        #[test]
        fn malformed_values_fall_back_to_defaults() {
            let result = Config::load(
                "target_chunk_send_rate=notanumber\nuse_async_portals=1\nuse_async_logging=TRUE\n",
                &[],
                false,
            );
            assert_eq!(result.config.target_chunk_send_rate, -1);
            assert!(result.config.use_async_portals); // "1" is not a boolean: default
            assert!(result.config.use_async_logging); // trimmed, case-insensitive true
        }

        #[test]
        fn last_assignment_wins_like_java_properties() {
            let result = Config::load(
                "target_chunk_send_rate=1\ntarget_chunk_send_rate=2\n",
                &[],
                false,
            );
            assert_eq!(result.config.target_chunk_send_rate, 2);
        }

        #[test]
        fn incompatibility_forces_boolean_off_with_upstream_wording() {
            let result = Config::load("", &[inc("use_async_portals")], false);
            assert!(!result.config.use_async_portals);
            assert_eq!(
            result.warnings,
            vec!["Forcing use_async_portals in vmp.properties to be disabled (defined in othermod@1.2.3) (you may override this with -Dvmp.ignoreIncompatibleConfig=true)".to_string()]
        );
            // the write-back keeps the parsed value, as upstream does
            assert!(
                result
                    .normalized_properties
                    .contains("use_async_portals=true")
            );
        }

        #[test]
        fn ignore_flag_skips_the_forcing() {
            let result = Config::load("", &[inc("use_async_portals")], true);
            assert!(result.config.use_async_portals);
            assert_eq!(
                result.warnings,
                vec![
                    "Ignoring incompatibility of use_async_portals (defined in othermod@1.2.3)"
                        .to_string()
                ]
            );
        }

        #[test]
        fn properties_reader_handles_separators_and_comments() {
            let entries =
                parse_properties("a=1\n! comment\n# comment\n  b : 2\n\tc 3\nd=spaced value\n");
            let find = |k: &str| {
                entries
                    .iter()
                    .find(|(key, _)| key == k)
                    .map(|(_, v)| v.clone())
            };
            assert_eq!(find("a").as_deref(), Some("1"));
            assert_eq!(find("b").as_deref(), Some("2"));
            assert_eq!(find("c").as_deref(), Some("3"));
            assert_eq!(find("d").as_deref(), Some("spaced value"));
            assert_eq!(entries.len(), 4);
        }

        #[test]
        fn escapes_and_continuations_parse() {
            let entries = parse_properties("path=C\\:\\\\dir\nmulti=one\\\n  two\n");
            let find = |k: &str| {
                entries
                    .iter()
                    .find(|(key, _)| key == k)
                    .map(|(_, v)| v.clone())
            };
            assert_eq!(find("path").as_deref(), Some("C:\\dir"));
            assert_eq!(find("multi").as_deref(), Some("onetwo"));
        }
    }
}
pub mod mcutil {
    // io.papermc.paper.util.MCUtil.getCoordinateKey/getCoordinateX/Z and the
    // ChunkPos.getPackedX/getPackedZ equivalents VMP builds its maps on.

    pub fn get_coordinate_key(x: i32, z: i32) -> i64 {
        ((x as i64) << 32) | ((z as i64) & 0xFFFF_FFFF)
    }

    pub fn get_coordinate_x(key: i64) -> i32 {
        (key >> 32) as i32
    }

    pub fn get_coordinate_z(key: i64) -> i32 {
        ((key << 32) >> 32) as i32
    }

    pub fn chunk_pos_packed_x(pos: i64) -> i32 {
        get_coordinate_x(pos)
    }

    pub fn chunk_pos_packed_z(pos: i64) -> i32 {
        get_coordinate_z(pos)
    }

    pub fn chunk_pos_as_long(x: i32, z: i32) -> i64 {
        get_coordinate_key(x, z)
    }
}
pub mod pool {
    // Port of com.ishland.vmp.common.util.SimpleObjectPool (SimpleObjectPool.java).
    // A fixed-size pool that pre-constructs `size` objects and hands them out;
    // alloc() past the size falls back to a plain construction (no initializer),
    // and release() is a no-op once every pooled object is already out.
    // Upstream guards the array with synchronized blocks; the port keeps the
    // same state machine for single-threaded use.

    pub struct SimpleObjectPool<T> {
        constructor: Box<dyn Fn() -> T>,
        initializer: Box<dyn Fn(&mut T)>,
        on_recycle: Box<dyn Fn(&mut T)>,
        size: usize,
        cached: Vec<Option<T>>,
        allocated_count: usize,
    }

    impl<T> SimpleObjectPool<T> {
        pub fn new(
            constructor: Box<dyn Fn() -> T>,
            initializer: Box<dyn Fn(&mut T)>,
            on_recycle: Box<dyn Fn(&mut T)>,
            size: usize,
        ) -> Self {
            assert!(size > 0, "size must be positive");
            Self {
                constructor,
                initializer,
                on_recycle,
                size,
                cached: Vec::new(),
                allocated_count: 0,
            }
        }

        fn init(&mut self) {
            if self.cached.is_empty() {
                self.cached = (0..self.size).map(|_| Some((self.constructor)())).collect();
            }
        }

        pub fn alloc(&mut self) -> T {
            self.init();
            if self.allocated_count >= self.size {
                // oversized, falling back to a normal alloc
                return (self.constructor)();
            }
            let ordinal = self.allocated_count;
            self.allocated_count += 1;
            let mut object = self.cached[ordinal].take().expect("pooled slot");
            (self.initializer)(&mut object);
            object
        }

        pub fn release(&mut self, mut object: T) {
            self.init();
            if self.allocated_count == 0 {
                return; // pool is full
            }
            (self.on_recycle)(&mut object);
            self.allocated_count -= 1;
            self.cached[self.allocated_count] = Some(object);
        }

        pub fn get_allocated_count(&self) -> usize {
            self.allocated_count
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn counting_pool(
            size: usize,
        ) -> (
            SimpleObjectPool<Vec<u32>>,
            std::rc::Rc<std::cell::Cell<usize>>,
        ) {
            let constructed = std::rc::Rc::new(std::cell::Cell::new(0usize));
            let c = constructed.clone();
            let pool = SimpleObjectPool::new(
                Box::new(move || {
                    c.set(c.get() + 1);
                    Vec::new()
                }),
                Box::new(|v: &mut Vec<u32>| v.push(1)),
                Box::new(|v: &mut Vec<u32>| v.clear()),
                size,
            );
            (pool, constructed)
        }

        #[test]
        fn alloc_reuses_released_objects() {
            let (mut pool, constructed) = counting_pool(2);
            let a = pool.alloc();
            let b = pool.alloc();
            assert_eq!(pool.get_allocated_count(), 2);
            assert_eq!(constructed.get(), 2);
            pool.release(a);
            let c = pool.alloc();
            assert_eq!(c, vec![1]); // initializer ran again on the recycled object
            assert_eq!(constructed.get(), 2); // no new construction
            drop(b);
        }

        #[test]
        fn oversized_alloc_falls_back_without_initializer() {
            let (mut pool, constructed) = counting_pool(1);
            let a = pool.alloc();
            let b = pool.alloc(); // oversized
            assert_eq!(b, Vec::<u32>::new()); // no initializer pass
            assert_eq!(constructed.get(), 2);
            pool.release(a);
            assert_eq!(pool.get_allocated_count(), 0);
            drop(b);
        }

        #[test]
        fn release_when_pool_is_full_is_a_no_op() {
            let (mut pool, constructed) = counting_pool(1);
            let a = pool.alloc();
            assert_eq!(pool.get_allocated_count(), 1);
            pool.release(a); // allocated_count wraps to 0, object stored back
            assert_eq!(pool.get_allocated_count(), 0);
            pool.release(Vec::new()); // pool already full: no-op
            assert_eq!(pool.get_allocated_count(), 0);
            assert_eq!(constructed.get(), 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_key_round_trip() {
        for (x, z) in [(0, 0), (1, -1), (-3, 7), (i32::MAX, i32::MIN), (-1, -1)] {
            let key = mcutil::get_coordinate_key(x, z);
            assert_eq!(mcutil::get_coordinate_x(key), x);
            assert_eq!(mcutil::get_coordinate_z(key), z);
        }
    }

    #[test]
    fn packed_pos_matches_chunk_pos_helpers() {
        let pos = mcutil::get_coordinate_key(-57, 1203);
        assert_eq!(mcutil::chunk_pos_packed_x(pos), -57);
        assert_eq!(mcutil::chunk_pos_packed_z(pos), 1203);
        assert_eq!(
            mcutil::get_coordinate_key(-57, 1203),
            mcutil::chunk_pos_as_long(-57, 1203)
        );
    }
}
