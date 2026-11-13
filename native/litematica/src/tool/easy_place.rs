use malilib::util::BlockPos;
use crate::placement::PlacementManager;
use crate::materials::MaterialList;
use crate::schematic::BlockState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EasyPlaceVerdict {
    Valid {
        expected_state: BlockState,
        required_item: String,
    },
    WrongItem {
        expected_state: BlockState,
        required_item: String,
        held_item: String,
    },
    AlreadySatisfied,
    NotInSchematic,
}

#[derive(Debug, Default, Clone)]
pub struct EasyPlace {
    pub enabled: bool,
    pub prevent_wrong_placements: bool,
}

impl EasyPlace {
    pub fn new() -> Self {
        Self {
            enabled: true,
            prevent_wrong_placements: true,
        }
    }

    pub fn evaluate_placement(
        &self,
        world_pos: BlockPos,
        current_world_state: &BlockState,
        held_item: &str,
        placement_mgr: &PlacementManager,
    ) -> EasyPlaceVerdict {
        if !self.enabled {
            return EasyPlaceVerdict::NotInSchematic;
        }

        let Some(expected_state) = placement_mgr.get_block_at_world_pos(world_pos) else {
            return EasyPlaceVerdict::NotInSchematic;
        };

        if expected_state.is_air() {
            return EasyPlaceVerdict::NotInSchematic;
        }

        if current_world_state == &expected_state {
            return EasyPlaceVerdict::AlreadySatisfied;
        }

        let required_item = MaterialList::block_to_item(&expected_state.name);

        if held_item == required_item {
            EasyPlaceVerdict::Valid {
                expected_state,
                required_item,
            }
        } else {
            EasyPlaceVerdict::WrongItem {
                expected_state,
                required_item,
                held_item: held_item.to_string(),
            }
        }
    }
}
