// Better Recipe Book -> Horizon native mod, slice 1: pure-logic core.
// Upstream: https://github.com/mrshmllow/BetterRecipeBook (Modrinth slug 'brb'),
// commit ea7c787 (2024-07-07), MIT (c) 2021 marshamallow. LICENSE carried verbatim
// in this mod's repo. Ported 1:1 from common/src/main/java/marsh/town/brb:
// config/{Config,NewRecipes,InstantCraft,AlternativeRecipes,Scrolling}.java,
// InstantCraftingManager.java, PinnedRecipeManager.java.
//
// Seam notes (game-side, not modeled here):
// - Config load/save goes through the game's config holder; upstream registers
//   load/save listeners that re-sync the instant-craft toggle button (GUI seam).
// - ItemStack identity (isSameItemSameComponents) is modeled as a comparable
//   CraftResult { item, components } where components is the serialized
//   component map ("" = none). Minecraft registry types stay out of the core.
// - onResultSlotUpdated's click is sent by the game glue as QUICK_MOVE, slot 0,
//   button 0, on the screen's containerId, exactly like upstream
//   gameMode.handleInventoryMouseClick.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MOD_ID: &str = "brb";
// Upstream: new File(gameDirectory, MOD_ID + ".pins") -- note: no .json extension.
pub const PINS_FILE: &str = "brb.pins";
// Upstream autoconfig @Config(name = "brb") -> config/brb.json.
pub const CONFIG_FILE: &str = "brb.json";

// ----- config/NewRecipes.java -----
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NewRecipes {
    pub unlock_all: bool,
    pub enable_bounce: bool,
}

impl Default for NewRecipes {
    fn default() -> Self {
        Self {
            unlock_all: true,
            enable_bounce: false,
        }
    }
}

// ----- config/InstantCraft.java -----
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstantCraft {
    pub show_button: bool,
    pub enabled: bool,
}

impl Default for InstantCraft {
    fn default() -> Self {
        Self {
            show_button: true,
            enabled: false,
        }
    }
}

// ----- config/AlternativeRecipes.java -----
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlternativeRecipes {
    pub on_hover: bool,
    pub no_grouped: bool,
}

impl Default for AlternativeRecipes {
    fn default() -> Self {
        Self {
            on_hover: true,
            no_grouped: false,
        }
    }
}

// ----- config/Scrolling.java -----
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Scrolling {
    pub enable_scrolling: bool,
    pub scroll_around: bool,
}

impl Default for Scrolling {
    fn default() -> Self {
        Self {
            enable_scrolling: true,
            scroll_around: false,
        }
    }
}

// ----- config/Config.java -----
// Upstream @Config(name = "brb"); autoconfig writes nested objects with the
// Java field names as-is (camelCase). Sections mirror the ConfigEntry categories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub enable_pinning: bool,
    pub keep_centered: bool,
    pub new_recipes: NewRecipes,
    pub instant_craft: InstantCraft,
    pub alternative_recipes: AlternativeRecipes,
    pub scrolling: Scrolling,
    pub settings_button: bool,
    pub enable_book: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable_pinning: true,
            keep_centered: false,
            new_recipes: NewRecipes::default(),
            instant_craft: InstantCraft::default(),
            alternative_recipes: AlternativeRecipes::default(),
            scrolling: Scrolling::default(),
            settings_button: true,
            enable_book: true,
        }
    }
}

impl Config {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("brb config serializes")
    }

    // Upstream: a broken config file makes autoconfig's Gson load fail, the
    // holder logs and keeps the default instance. Same here.
    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).unwrap_or_default()
    }
}

// ----- InstantCraftingManager.java -----
// ItemStack.isSameItemSameComponents modeled as value equality on CraftResult.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftResult {
    pub item: String,
    // Serialized component map; empty string = default components.
    pub components: String,
}

#[derive(Debug, Clone, Default)]
pub struct InstantCraftingManager {
    // last clicked recipe id before updateCollections runs
    pub last_clicked_recipe: Option<String>,
    // updated by the recipe-book page hook (GUI seam)
    pub last_hovered_collection: Option<String>,
    // instant-craft toggle button handle (GUI seam; re-synced on config reload)
    pub last_instant_craft_button: Option<()>,
    pub last_craft_result: Option<CraftResult>,
    pub last_container_id: i64,
    // mirror of config.instantCraft.enabled; the config holder keeps the truth
    pub enabled: bool,
}

impl InstantCraftingManager {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_container_id: -1,
            ..Default::default()
        }
    }

    // upstream recipeClicked(recipe, registryAccess)
    pub fn recipe_clicked(
        &mut self,
        recipe_id: &str,
        result: CraftResult,
        container_id: i64,
        screen_is_container: bool,
    ) {
        if !self.enabled {
            // instant craft off: clear any pending instant craft
            self.last_craft_result = None;
            return;
        }
        if !screen_is_container {
            return;
        }
        // keep buttons stationary: remember which recipe was clicked
        self.last_clicked_recipe = Some(recipe_id.to_string());
        // set last craft for onResultSlotUpdated
        self.last_craft_result = Some(result);
        // the server may never set the result item, so results from other
        // container ids are ignored
        self.last_container_id = container_id;
    }

    // upstream onResultSlotUpdated(itemStack). Returns true when the glue must
    // send the QUICK_MOVE click (slot 0, button 0, this containerId); the click
    // itself is the game seam.
    pub fn on_result_slot_updated(
        &mut self,
        stack: &CraftResult,
        container_id: i64,
        game_mode_present: bool,
        screen_is_container: bool,
    ) -> bool {
        if !self.enabled || !game_mode_present || !screen_is_container {
            return false;
        }
        let Some(last) = &self.last_craft_result else {
            return false;
        };
        if *last != *stack || self.last_container_id != container_id {
            return false;
        }
        self.last_craft_result = None;
        true
    }

    // upstream toggleEnabled(): flips config.instantCraft.enabled and saves via
    // the config holder (save is the game-side seam).
    pub fn toggle_enabled(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ----- PinnedRecipeManager.java -----
// ResourceLocation modeled as its string form ("namespace:path").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinnedRecipeManager {
    pub pinned: HashSet<String>,
}

impl PinnedRecipeManager {
    // upstream read(): parse brb.pins; on any error log and fall back to an
    // empty set (Gson leaves the field null when the file is absent or broken).
    pub fn read_from_str(&mut self, json: &str) -> Result<(), serde_json::Error> {
        self.pinned = match serde_json::from_str::<Vec<String>>(json) {
            Ok(list) => list.into_iter().collect(),
            Err(e) => {
                self.pinned = HashSet::new();
                return Err(e);
            }
        };
        Ok(())
    }

    // upstream store(): write brb.pins as a JSON array (Gson serializes the
    // HashSet as an array; element order is unspecified there and here).
    pub fn store_to_string(&self) -> String {
        let list: Vec<&String> = self.pinned.iter().collect();
        serde_json::to_string(&list).expect("pins serialize")
    }

    // upstream addOrRemoveFavourite(target): if any pinned id is one of the
    // target's recipes, unpin exactly that id; otherwise pin every recipe of
    // the target. Returns true when the set changed and must be stored.
    pub fn add_or_remove_favourite(&mut self, recipe_ids: &[String]) -> bool {
        let matched = self
            .pinned
            .iter()
            .find(|id| recipe_ids.iter().any(|r| r == *id))
            .cloned();
        if let Some(id) = matched {
            self.pinned.remove(&id);
            return true;
        }
        let before = self.pinned.len();
        self.pinned.extend(recipe_ids.iter().cloned());
        self.pinned.len() != before
    }

    // upstream has(target): true when any pinned id is in the pinnable's ids.
    pub fn has(&self, target_recipe_ids: &[String]) -> bool {
        self.pinned
            .iter()
            .any(|id| target_recipe_ids.iter().any(|r| r == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_upstream() {
        let c = Config::default();
        assert!(c.enable_pinning);
        assert!(!c.keep_centered);
        assert_eq!(
            c.new_recipes,
            NewRecipes {
                unlock_all: true,
                enable_bounce: false
            }
        );
        assert_eq!(
            c.instant_craft,
            InstantCraft {
                show_button: true,
                enabled: false
            }
        );
        assert_eq!(
            c.alternative_recipes,
            AlternativeRecipes {
                on_hover: true,
                no_grouped: false
            }
        );
        assert_eq!(
            c.scrolling,
            Scrolling {
                enable_scrolling: true,
                scroll_around: false
            }
        );
        assert!(c.settings_button);
        assert!(c.enable_book);
    }

    #[test]
    fn config_json_round_trip_camel_case() {
        let mut c = Config::default();
        c.instant_craft.enabled = true;
        c.scrolling.scroll_around = true;
        let json = c.to_json();
        assert!(json.contains("\"newRecipes\""));
        assert!(json.contains("\"instantCraft\""));
        assert!(json.contains("\"alternativeRecipes\""));
        assert!(json.contains("\"enablePinning\""));
        assert_eq!(Config::from_json(&json), c);
    }

    #[test]
    fn config_broken_json_falls_back_to_defaults() {
        let c = Config::from_json("{ not json");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn instant_craft_disabled_clears_pending_result() {
        let mut m = InstantCraftingManager::new(false);
        m.last_craft_result = Some(CraftResult {
            item: "minecraft:stick".into(),
            components: String::new(),
        });
        m.recipe_clicked(
            "minecraft:x",
            CraftResult {
                item: "a".into(),
                components: String::new(),
            },
            7,
            true,
        );
        assert!(m.last_craft_result.is_none());
        assert!(m.last_clicked_recipe.is_none());
    }

    #[test]
    fn instant_craft_requires_container_screen() {
        let mut m = InstantCraftingManager::new(true);
        m.recipe_clicked(
            "minecraft:x",
            CraftResult {
                item: "a".into(),
                components: String::new(),
            },
            7,
            false,
        );
        assert!(m.last_clicked_recipe.is_none());
        assert!(m.last_craft_result.is_none());
        assert_eq!(m.last_container_id, -1);
    }

    #[test]
    fn instant_craft_stores_state_on_click() {
        let mut m = InstantCraftingManager::new(true);
        let r = CraftResult {
            item: "minecraft:stick".into(),
            components: "".into(),
        };
        m.recipe_clicked("minecraft:planks", r.clone(), 7, true);
        assert_eq!(m.last_clicked_recipe.as_deref(), Some("minecraft:planks"));
        assert_eq!(m.last_craft_result, Some(r));
        assert_eq!(m.last_container_id, 7);
    }

    #[test]
    fn result_update_clicks_when_matching() {
        let mut m = InstantCraftingManager::new(true);
        let r = CraftResult {
            item: "minecraft:stick".into(),
            components: "".into(),
        };
        m.recipe_clicked("minecraft:planks", r.clone(), 7, true);
        assert!(m.on_result_slot_updated(&r, 7, true, true));
        assert!(m.last_craft_result.is_none());
    }

    #[test]
    fn result_update_ignores_other_container_id() {
        let mut m = InstantCraftingManager::new(true);
        let r = CraftResult {
            item: "minecraft:stick".into(),
            components: "".into(),
        };
        m.recipe_clicked("minecraft:planks", r.clone(), 7, true);
        assert!(!m.on_result_slot_updated(&r, 9, true, true));
        assert_eq!(m.last_craft_result, Some(r));
    }

    #[test]
    fn result_update_requires_same_item_and_components() {
        let mut m = InstantCraftingManager::new(true);
        m.recipe_clicked(
            "minecraft:planks",
            CraftResult {
                item: "minecraft:stick".into(),
                components: "".into(),
            },
            7,
            true,
        );
        // different components on the same item: not the same stack
        assert!(!m.on_result_slot_updated(
            &CraftResult {
                item: "minecraft:stick".into(),
                components: "{a:1}".into()
            },
            7,
            true,
            true,
        ));
        // same item and components but no game mode: upstream returns early
        assert!(!m.on_result_slot_updated(
            &CraftResult {
                item: "minecraft:stick".into(),
                components: "".into()
            },
            7,
            false,
            true,
        ));
        assert!(m.on_result_slot_updated(
            &CraftResult {
                item: "minecraft:stick".into(),
                components: "".into()
            },
            7,
            true,
            true,
        ));
    }

    #[test]
    fn toggle_flips_state() {
        let mut m = InstantCraftingManager::new(false);
        assert!(!m.is_enabled());
        assert!(m.toggle_enabled());
        assert!(m.is_enabled());
        assert!(!m.toggle_enabled());
    }

    #[test]
    fn pins_read_valid_array() {
        let mut p = PinnedRecipeManager::default();
        p.read_from_str("[\"minecraft:stick\",\"minecraft:apple\"]")
            .unwrap();
        assert_eq!(p.pinned.len(), 2);
        assert!(p.pinned.contains("minecraft:stick"));
    }

    #[test]
    fn pins_read_broken_falls_back_to_empty() {
        let mut p = PinnedRecipeManager::default();
        assert!(p.read_from_str("nope").is_err());
        assert!(p.pinned.is_empty());
    }

    #[test]
    fn pins_read_absent_file_means_empty() {
        // upstream: file missing -> gson leaves null -> empty set
        let mut p = PinnedRecipeManager::default();
        p.read_from_str("").unwrap_err();
        assert!(p.pinned.is_empty());
    }

    #[test]
    fn pins_store_round_trip() {
        let mut p = PinnedRecipeManager::default();
        p.add_or_remove_favourite(&["minecraft:stick".into(), "minecraft:apple".into()]);
        let json = p.store_to_string();
        let mut q = PinnedRecipeManager::default();
        q.read_from_str(&json).unwrap();
        assert_eq!(q.pinned, p.pinned);
    }

    #[test]
    fn pins_add_then_remove_favourite() {
        let mut p = PinnedRecipeManager::default();
        let ids = vec!["minecraft:stick".into(), "minecraft:apple".into()];
        assert!(p.add_or_remove_favourite(&ids));
        assert!(p.has(&ids));
        // second call on a matching collection unpins exactly the matched id;
        // the other recipe stays pinned, so the collection is still has()-ed
        assert!(p.add_or_remove_favourite(&ids));
        assert_eq!(
            p.pinned,
            ["minecraft:apple".to_string()].into_iter().collect()
        );
        assert!(p.has(&ids));
    }

    #[test]
    fn pins_remove_only_matched_id() {
        let mut p = PinnedRecipeManager::default();
        p.pinned.insert("minecraft:dirt".into());
        let ids = vec!["minecraft:dirt".into(), "minecraft:stone".into()];
        assert!(p.add_or_remove_favourite(&ids));
        // dirt was unpinned; stone was NOT added (upstream returns after remove)
        assert!(!p.pinned.contains("minecraft:stone"));
        assert!(p.pinned.is_empty());
    }

    #[test]
    fn pins_has_partial_match() {
        let mut p = PinnedRecipeManager::default();
        p.pinned.insert("minecraft:stick".into());
        assert!(p.has(&["minecraft:stick".into(), "minecraft:apple".into()]));
        assert!(!p.has(&["minecraft:apple".into()]));
    }

    #[test]
    fn file_names_match_upstream() {
        assert_eq!(PINS_FILE, "brb.pins");
        assert_eq!(CONFIG_FILE, "brb.json");
        assert_eq!(MOD_ID, "brb");
    }
}
