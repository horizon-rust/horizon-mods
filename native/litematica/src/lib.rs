pub mod schematic;
pub mod placement;
pub mod verifier;
pub mod materials;
pub mod selection;
pub mod tool;
pub mod render;
pub mod gui;
pub mod config;

pub use schematic::{BlockState, BlockStatePalette, LitematicaSchematic, PackedBitArray, SubRegion, SchematicConverter};
pub use placement::{PlacementManager, SchematicPlacement, SubRegionPlacement};
pub use verifier::{MismatchEntry, MismatchType, SchematicVerifier, VerificationResult};
pub use materials::{MaterialEntry, MaterialList};
pub use selection::{AreaSelection, SelectionManager};
pub use tool::{EasyPlace, EasyPlaceVerdict, ToolMode};
pub use render::{GhostOverlayMesh, LineVertex, WireframeBuilder};
pub use gui::{LitematicaMainMenu, MainMenuTab};

use std::sync::{Mutex, OnceLock};

static PLACEMENT_MANAGER: OnceLock<Mutex<PlacementManager>> = OnceLock::new();
static SELECTION_MANAGER: OnceLock<Mutex<SelectionManager>> = OnceLock::new();
static EASY_PLACE: OnceLock<Mutex<EasyPlace>> = OnceLock::new();

pub fn global_placement_manager() -> &'static Mutex<PlacementManager> {
    PLACEMENT_MANAGER.get_or_init(|| Mutex::new(PlacementManager::new()))
}

pub fn global_selection_manager() -> &'static Mutex<SelectionManager> {
    SELECTION_MANAGER.get_or_init(|| Mutex::new(SelectionManager::new()))
}

pub fn global_easy_place() -> &'static Mutex<EasyPlace> {
    EASY_PLACE.get_or_init(|| Mutex::new(EasyPlace::new()))
}

pub fn init() {
    log::info!("Initialized Litematica (Rust / Horizon)");
    let mut config_mgr = malilib::global_config().lock().unwrap();
    config::register_litematica_configs(&mut config_mgr);

    let mut hotkey_mgr = malilib::global_hotkeys().lock().unwrap();
    config::register_litematica_hotkeys(&mut hotkey_mgr);
}
