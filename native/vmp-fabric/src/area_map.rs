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
            let mut keys: Vec<i64> =
                Vec::with_capacity(((2 * view_distance + 1) * (2 * view_distance + 1)) as usize);
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
            let mut keys: Vec<i64> =
                Vec::with_capacity(((2 * view_distance + 1) * (2 * view_distance + 1)) as usize);
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

fn is_in_range(x_lower: i32, x_higher: i32, z_lower: i32, z_higher: i32, x: i32, z: i32) -> bool {
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
