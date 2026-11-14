//! Upstream `common/network/` packets, `common/network/server/ClientPacketHandler.java`
//! and the packet flows, translated 1:1. The Minecraft `StreamCodec`/wire
//! layer is a host seam: packets are the record fields, each `TYPE` is the
//! namespaced id upstream registers, and the handlers carry the logic.

use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use crate::block_entity_recipe_data::AbstractBlockEntityRecipeData;
use crate::block_entity_ticker::BlockEntityTicker;
use crate::player_recipe_data::PlayerRecipeData;
use crate::recipe_cache::RecipeHolder;
use crate::recipe_pair::{Identifier, ItemStack, RecipePair};

/// Upstream `PolymorphApi.MOD_ID`.
pub const MOD_ID: &str = "polymorph";

/// `PolymorphApi.MOD_ID` namespaced type id, standing in for the Java
/// `Type<...>` constants.
pub fn type_id(name: &str) -> Identifier {
    Identifier::new(MOD_ID, name)
}

/// Host seam for the receiving `ServerPlayer`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ServerPlayerRef {
    pub token: u64,
    pub container_menu: u64,
    pub inventory: u64,
}

/// The capability handed over for `CPacketBlockEntityListener`: the owner
/// token (the ticker map key), the block-entity liveness seam and the data.
pub struct BlockEntityData {
    pub owner: u64,
    pub is_alive: Rc<dyn Fn() -> bool>,
    pub data: AbstractBlockEntityRecipeData<()>,
}

/// Host seams for the server side of the clientbound packets.
pub trait ServerPacketEnv {
    /// The receiving player. The host always has one; the explicit null
    /// guard in `CPacketBlockEntityListener` is impossible on a non-null
    /// host.
    fn player(&self) -> ServerPlayerRef;
    /// `player.level().getRecipeManager().byKey(...)`.
    fn by_key(&self, id: &Identifier) -> Option<RecipeHolder>;
    /// `PolymorphApi.getInstance().getPlayerRecipeData(player)`.
    fn player_recipe_data(&mut self) -> Option<&mut PlayerRecipeData>;
    /// `PolymorphApi.getInstance().getBlockEntityRecipeData(container)`,
    /// handed over for the ticker registration.
    fn block_entity_recipe_data(&mut self, container: u64) -> Option<BlockEntityData>;
    /// `PolymorphIntegrations.selectRecipe(...)`.
    fn integrations_select(&mut self, owner: u64, container: u64, recipe: &RecipeHolder);
    /// `AbstractContainerMenu#slotsChanged(player.getInventory())`.
    fn slots_changed(&mut self, container: u64, inventory: u64);
    /// `container instanceof ItemCombinerMenu`.
    fn is_combiner_menu(&self, container: u64) -> bool;
    /// `((ItemCombinerMenu) container).createResult()`.
    fn create_result(&mut self, container: u64);
    /// `container.broadcastChanges()`.
    fn broadcast_changes(&mut self, container: u64);
    /// `api.getNetwork().sendUpdatePreviewS2C(player)`.
    fn send_update_preview_s2c(&mut self, player: u64);
    /// `api.getNetwork().sendRecipesListS2C(player, pairs, selected)`.
    fn send_recipes_list_s2c(
        &mut self,
        player: u64,
        pairs: &BTreeSet<RecipePair>,
        selected: Option<&Identifier>,
    );
}

/// Upstream record `CPacketPlayerRecipeSelection(ResourceLocation recipe)`.
pub struct CPacketPlayerRecipeSelection {
    pub recipe: Identifier,
}

impl CPacketPlayerRecipeSelection {
    pub fn type_() -> Identifier {
        type_id("player_recipe_selection")
    }

    /// `handle(packet, ServerPlayer)`.
    pub fn handle(&self, env: &mut dyn ServerPacketEnv) {
        let player = env.player();
        let container = player.container_menu;
        if let Some(recipe) = env.by_key(&self.recipe) {
            if let Some(recipe_data) = env.player_recipe_data() {
                recipe_data.select_recipe(recipe.clone());
            }
            env.integrations_select(player.token, container, &recipe);
            env.slots_changed(container, player.inventory);
            if env.is_combiner_menu(container) {
                env.create_result(container);
                env.broadcast_changes(container);
                env.send_update_preview_s2c(player.token);
            }
        }
    }
}

/// Upstream record `CPacketBlockEntityListener(boolean add)`.
pub struct CPacketBlockEntityListener {
    pub add: bool,
}

impl CPacketBlockEntityListener {
    pub fn type_() -> Identifier {
        type_id("block_entity_listener")
    }

    /// `handle(packet, ServerPlayer)`.
    pub fn handle(&self, env: &mut dyn ServerPacketEnv) {
        let player = env.player();
        if self.add {
            let container = player.container_menu;
            if let Some(be) = env.block_entity_recipe_data(container) {
                BlockEntityTicker::add(player.token, be.owner, be.data, be.is_alive);
                let (pairs, selected) = BlockEntityTicker
                    ::read(be.owner, |data| {
                        (
                            if data.is_empty() {
                                BTreeSet::new()
                            } else {
                                data.get_recipes_list().clone()
                            },
                            data.get_selected_recipe().map(|r| r.id.clone()),
                        )
                    })
                    .unwrap_or((BTreeSet::new(), None));
                env.send_recipes_list_s2c(player.token, &pairs, selected.as_ref());
            }
        } else {
            BlockEntityTicker::remove(player.token);
        }
    }
}

/// Upstream record `CPacketPersistentRecipeSelection(ResourceLocation recipe)`.
pub struct CPacketPersistentRecipeSelection {
    pub recipe: Identifier,
}

impl CPacketPersistentRecipeSelection {
    pub fn type_() -> Identifier {
        type_id("persistent_recipe_selection")
    }

    /// `handle(packet, ServerPlayer)`: the command-sender world's recipe
    /// manager resolves the id, the open menu's block entity gets the
    /// selection plus the integration hook.
    pub fn handle(&self, env: &mut dyn ServerPacketEnv) {
        if let Some(recipe) = env.by_key(&self.recipe) {
            let container = env.player().container_menu;
            let mut selected = false;
            if let Some(mut recipe_data) = env.block_entity_recipe_data(container) {
                let owner = recipe_data.owner;
                recipe_data.data.select_recipe(recipe.clone());
                env.integrations_select(owner, container, &recipe);
                selected = true;
            }
            let _ = selected;
        }
    }
}

/// Upstream record `SPacketPlayerRecipeSync(Optional<HashSet<IRecipePair>>,
/// Optional<ResourceLocation>)`.
pub struct SPacketPlayerRecipeSync {
    pub recipe_list: Option<HashSet<RecipePair>>,
    pub selected: Option<Identifier>,
}

impl SPacketPlayerRecipeSync {
    pub fn type_() -> Identifier {
        type_id("recipe_sync")
    }

    /// `handle(packet)` on the client.
    pub fn handle(self, env: &mut dyn ClientPacketEnv) {
        ClientPacketHandler::handle_player_recipe_sync(self, env);
    }
}

/// Upstream record `SPacketRecipesList(Optional<HashSet<IRecipePair>>,
/// Optional<ResourceLocation>)`.
pub struct SPacketRecipesList {
    pub recipe_list: Option<HashSet<RecipePair>>,
    pub selected: Option<Identifier>,
}

impl SPacketRecipesList {
    pub fn type_() -> Identifier {
        type_id("recipes_list")
    }

    /// `handle(packet)` on the client.
    pub fn handle(self, env: &mut dyn ClientPacketEnv) {
        ClientPacketHandler::handle_recipes_list(self, env);
    }
}

/// Upstream record `SPacketHighlightRecipe(ResourceLocation recipe)`.
pub struct SPacketHighlightRecipe {
    pub recipe: Identifier,
}

impl SPacketHighlightRecipe {
    pub fn type_() -> Identifier {
        type_id("highlight_recipe")
    }

    /// `handle(packet)` on the client.
    pub fn handle(self, env: &mut dyn ClientPacketEnv) {
        ClientPacketHandler::handle_highlight_recipe(self, env);
    }
}

/// Upstream class `SPacketUpdatePreview` (a singleton unit packet).
pub struct SPacketUpdatePreview;

impl SPacketUpdatePreview {
    pub const INSTANCE: SPacketUpdatePreview = SPacketUpdatePreview;

    pub fn type_() -> Identifier {
        type_id("update_preview")
    }

    /// `handle(packet)` on the client.
    pub fn handle(self, env: &mut dyn ClientPacketEnv) {
        ClientPacketHandler::handle_update_preview(self, env);
    }
}

/// Host seam for the client side: the `LocalPlayer`, the player capability,
/// the recipe manager lookup, the recipes widget and the smithing preview.
pub trait ClientPacketEnv {
    /// `Minecraft.getInstance().player`.
    fn client_player(&self) -> Option<u64>;
    /// `PolymorphApi.getInstance().getPlayerRecipeData(clientPlayer)`.
    fn player_recipe_data(&mut self) -> Option<&mut PlayerRecipeData>;
    /// `clientPlayer.level().getRecipeManager().byKey(...)`.
    fn by_key(&self, id: &Identifier) -> Option<RecipeHolder>;
    /// `RecipesWidget.get()`.
    fn recipes_widget(&mut self) -> Option<&mut dyn RecipesWidget>;
    /// `RecipesWidget.enqueueRecipesList(...)`.
    fn enqueue_recipes_list(&mut self, pairs: BTreeSet<RecipePair>, selected: Option<Identifier>);
    /// `mc.screen instanceof SmithingScreen`.
    fn screen_is_smithing(&self) -> bool;
    /// `smithingScreen.getMenu().getSlot(3).getItem()`.
    fn smithing_slot_item(&self) -> ItemStack;
    /// `AccessorSmithingScreen.callUpdateArmorStandPreview(stack)`.
    fn update_armor_stand_preview(&mut self, stack: ItemStack);
}

/// Host seam for `IRecipesWidget`.
pub trait RecipesWidget {
    /// `widget.setRecipesList(pairs, selected)`.
    fn set_recipes_list(&mut self, pairs: &BTreeSet<RecipePair>, selected: Option<&Identifier>);
    /// `widget.highlightRecipe(recipe)`.
    fn highlight_recipe(&mut self, recipe: &Identifier);
}

/// Upstream `common/network/server/ClientPacketHandler.java`.
pub struct ClientPacketHandler;

impl ClientPacketHandler {
    /// `handle(SPacketPlayerRecipeSync)`.
    pub fn handle_player_recipe_sync(
        packet: SPacketPlayerRecipeSync,
        env: &mut dyn ClientPacketEnv,
    ) {
        if env.client_player().is_some() {
            // Resolve the selection through the recipe manager first; the
            // upstream flatMap runs before `setSelectedRecipe`.
            let selected = packet
                .selected
                .as_ref()
                .and_then(|id| env.by_key(id));
            if let Some(recipe_data) = env.player_recipe_data() {
                recipe_data.set_recipes_list(sort(packet.recipe_list.unwrap_or_default()));
                if let Some(recipe) = selected {
                    recipe_data.set_selected_recipe(recipe);
                }
            }
        }
    }

    /// `handle(SPacketRecipesList)`.
    pub fn handle_recipes_list(packet: SPacketRecipesList, env: &mut dyn ClientPacketEnv) {
        if env.client_player().is_some() {
            let pairs = sort(packet.recipe_list.unwrap_or_default());
            match env.recipes_widget() {
                Some(widget) => widget.set_recipes_list(&pairs, packet.selected.as_ref()),
                None => env.enqueue_recipes_list(pairs, packet.selected.clone()),
            }
        }
    }

    /// `handle(SPacketHighlightRecipe)`.
    pub fn handle_highlight_recipe(packet: SPacketHighlightRecipe, env: &mut dyn ClientPacketEnv) {
        if env.client_player().is_some() {
            if let Some(widget) = env.recipes_widget() {
                widget.highlight_recipe(&packet.recipe);
            }
        }
    }

    /// `handle(SPacketUpdatePreview)`.
    pub fn handle_update_preview(_packet: SPacketUpdatePreview, env: &mut dyn ClientPacketEnv) {
        if env.screen_is_smithing() {
            let stack = env.smithing_slot_item();
            env.update_armor_stand_preview(stack);
        }
    }
}

/// `sort(HashSet<IRecipePair>)`: the wire set is sorted into a TreeSet.
fn sort(set: HashSet<RecipePair>) -> BTreeSet<RecipePair> {
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe_cache::Recipe;

    struct EmptyRecipe;
    impl Recipe for EmptyRecipe {
        fn result_item(&self) -> Option<ItemStack> {
            None
        }
        fn is_custom(&self) -> bool {
            false
        }
        fn assemble(&self, _input: &crate::recipe_cache::RecipeInput) -> ItemStack {
            ItemStack {
                item: Identifier::new("test", "result"),
                count: 1,
                components: Default::default(),
            }
        }
    }

    fn holder(id: &str) -> RecipeHolder {
        let (ns, path) = id.split_once(':').unwrap();
        RecipeHolder {
            id: Identifier::new(ns, path),
            recipe: Rc::new(EmptyRecipe),
        }
    }

    fn id(s: &str) -> Identifier {
        let (ns, path) = s.split_once(':').unwrap();
        Identifier::new(ns, path)
    }

    fn pair(recipe: &str) -> RecipePair {
        RecipePair {
            resource_location: id(recipe),
            output: ItemStack {
                item: id(&format!("test:out_{}", recipe.replace(':', "_"))),
                count: 1,
                components: Default::default(),
            },
        }
    }

    #[derive(Default)]
    struct TestServerEnv {
        player: ServerPlayerRef,
        registry: Vec<RecipeHolder>,
        player_data: Option<PlayerRecipeData>,
        block_entity: Option<BlockEntityData>,
        integrations: Vec<(u64, u64, Identifier)>,
        slots_changed_count: u32,
        combiner: bool,
        results: u32,
        broadcasts: u32,
        previews: u32,
        sent_lists: Vec<(u64, BTreeSet<RecipePair>, Option<Identifier>)>,
    }

    impl TestServerEnv {
        fn new() -> Self {
            Self {
                player: ServerPlayerRef { token: 1, container_menu: 100, inventory: 7 },
                ..Default::default()
            }
        }
        fn with_player_data(mut self) -> Self {
            self.player_data = Some(PlayerRecipeData::new(
                crate::player_recipe_data::PlayerHandle {
                    token: 1,
                    tick_count: 0,
                    container_menu: 100,
                    is_server_player: true,
                    is_client_side: false,
                },
            ));
            self
        }
        fn with_block_entity(mut self) -> Self {
            self.block_entity = Some(BlockEntityData {
                owner: 55,
                is_alive: Rc::new(|| true),
                data: AbstractBlockEntityRecipeData::new((), Vec::new),
            });
            self
        }
    }

    impl ServerPacketEnv for TestServerEnv {
        fn player(&self) -> ServerPlayerRef {
            self.player
        }
        fn by_key(&self, id: &Identifier) -> Option<RecipeHolder> {
            self.registry.iter().find(|h| &h.id == id).cloned()
        }
        fn player_recipe_data(&mut self) -> Option<&mut PlayerRecipeData> {
            self.player_data.as_mut()
        }
        fn block_entity_recipe_data(&mut self, _container: u64) -> Option<BlockEntityData> {
            self.block_entity.take()
        }
        fn integrations_select(&mut self, owner: u64, container: u64, recipe: &RecipeHolder) {
            self.integrations.push((owner, container, recipe.id.clone()));
        }
        fn slots_changed(&mut self, _container: u64, _inventory: u64) {
            self.slots_changed_count += 1;
        }
        fn is_combiner_menu(&self, _container: u64) -> bool {
            self.combiner
        }
        fn create_result(&mut self, _container: u64) {
            self.results += 1;
        }
        fn broadcast_changes(&mut self, _container: u64) {
            self.broadcasts += 1;
        }
        fn send_update_preview_s2c(&mut self, _player: u64) {
            self.previews += 1;
        }
        fn send_recipes_list_s2c(
            &mut self,
            player: u64,
            pairs: &BTreeSet<RecipePair>,
            selected: Option<&Identifier>,
        ) {
            self.sent_lists
                .push((player, pairs.clone(), selected.cloned()));
        }
    }

    #[test]
    fn player_selection_selects_syncs_and_refreshes() {
        let mut env = TestServerEnv::new().with_player_data();
        env.combiner = true;
        env.registry.push(holder("test:stick"));
        CPacketPlayerRecipeSelection { recipe: id("test:stick") }.handle(&mut env);
        assert_eq!(
            env.player_data.as_ref().unwrap().get_selected_recipe().map(|h| h.id.clone()),
            Some(id("test:stick"))
        );
        assert_eq!(
            env.integrations,
            vec![(1, 100, id("test:stick"))],
            "integration hook gets the player token, container and recipe"
        );
        assert_eq!(env.slots_changed_count, 1);
        assert_eq!(env.results, 1);
        assert_eq!(env.broadcasts, 1);
        assert_eq!(env.previews, 1, "combiner menu sends the preview update");
    }

    #[test]
    fn player_selection_skips_everything_on_an_unknown_recipe() {
        let mut env = TestServerEnv::new().with_player_data();
        CPacketPlayerRecipeSelection { recipe: id("test:missing") }.handle(&mut env);
        assert_eq!(env.slots_changed_count, 0);
        assert!(env.integrations.is_empty());
    }

    #[test]
    fn player_selection_without_a_capability_still_runs_integrations_and_slots() {
        let mut env = TestServerEnv::new();
        env.registry.push(holder("test:stick"));
        CPacketPlayerRecipeSelection { recipe: id("test:stick") }.handle(&mut env);
        assert_eq!(env.slots_changed_count, 1);
        assert_eq!(env.integrations.len(), 1);
    }

    #[test]
    fn block_entity_listener_add_registers_and_sends_the_list() {
        BlockEntityTicker::clear();
        let mut env = TestServerEnv::new().with_block_entity();
        CPacketBlockEntityListener { add: true }.handle(&mut env);
        assert_eq!(BlockEntityTicker::entry_count(), 1, "the BE is tickable");
        assert_eq!(
            BlockEntityTicker::read(55, |d| d.get_listeners()),
            Some(vec![1])
        );
        assert_eq!(
            env.sent_lists,
            vec![(1, BTreeSet::new(), None)],
            "an empty input sends an empty list with no selection"
        );
    }

    #[test]
    fn block_entity_listener_remove_drops_the_listener() {
        BlockEntityTicker::clear();
        let mut env = TestServerEnv::new().with_block_entity();
        CPacketBlockEntityListener { add: true }.handle(&mut env);
        CPacketBlockEntityListener { add: false }.handle(&mut env);
        assert_eq!(BlockEntityTicker::entry_count(), 0);
        assert_eq!(env.sent_lists.len(), 1, "only the add send went out");
    }

    #[test]
    fn persistent_selection_lands_on_the_block_entity_data() {
        let mut env = TestServerEnv::new().with_block_entity();
        env.registry.push(holder("test:trim"));
        CPacketPersistentRecipeSelection { recipe: id("test:trim") }.handle(&mut env);
        assert_eq!(
            env.integrations,
            vec![(55, 100, id("test:trim"))],
            "the owner passed to the integration hook is the block entity"
        );
    }

    #[test]
    fn persistent_selection_with_no_block_entity_does_nothing() {
        let mut env = TestServerEnv::new();
        env.registry.push(holder("test:trim"));
        CPacketPersistentRecipeSelection { recipe: id("test:trim") }.handle(&mut env);
        assert!(env.integrations.is_empty());
    }

    // Client side.

    struct TestWidget {
        sets: Vec<(BTreeSet<RecipePair>, Option<Identifier>)>,
        highlights: Vec<Identifier>,
    }

    impl RecipesWidget for TestWidget {
        fn set_recipes_list(&mut self, pairs: &BTreeSet<RecipePair>, selected: Option<&Identifier>) {
            self.sets.push((pairs.clone(), selected.cloned()));
        }
        fn highlight_recipe(&mut self, recipe: &Identifier) {
            self.highlights.push(recipe.clone());
        }
    }

    struct TestClientEnv {
        player: Option<u64>,
        player_data: PlayerRecipeData,
        registry: Vec<RecipeHolder>,
        widget: Option<TestWidget>,
        enqueued: Vec<(BTreeSet<RecipePair>, Option<Identifier>)>,
        smithing: bool,
        previews: Vec<ItemStack>,
    }

    impl TestClientEnv {
        fn new() -> Self {
            Self {
                player: Some(9),
                player_data: PlayerRecipeData::new(crate::player_recipe_data::PlayerHandle {
                    token: 9,
                    tick_count: 0,
                    container_menu: 0,
                    is_server_player: false,
                    is_client_side: true,
                }),
                registry: Vec::new(),
                widget: Some(TestWidget { sets: Vec::new(), highlights: Vec::new() }),
                enqueued: Vec::new(),
                smithing: false,
                previews: Vec::new(),
            }
        }
    }

    impl ClientPacketEnv for TestClientEnv {
        fn client_player(&self) -> Option<u64> {
            self.player
        }
        fn player_recipe_data(&mut self) -> Option<&mut PlayerRecipeData> {
            Some(&mut self.player_data)
        }
        fn by_key(&self, id: &Identifier) -> Option<RecipeHolder> {
            self.registry.iter().find(|h| &h.id == id).cloned()
        }
        fn recipes_widget(&mut self) -> Option<&mut dyn RecipesWidget> {
            self.widget.as_mut().map(|w| w as &mut dyn RecipesWidget)
        }
        fn enqueue_recipes_list(
            &mut self,
            pairs: BTreeSet<RecipePair>,
            selected: Option<Identifier>,
        ) {
            self.enqueued.push((pairs, selected));
        }
        fn screen_is_smithing(&self) -> bool {
            self.smithing
        }
        fn smithing_slot_item(&self) -> ItemStack {
            ItemStack { item: id("test:helm"), count: 1, components: Default::default() }
        }
        fn update_armor_stand_preview(&mut self, stack: ItemStack) {
            self.previews.push(stack);
        }
    }

    #[test]
    fn player_sync_sets_the_sorted_list_and_resolved_selection() {
        let mut env = TestClientEnv::new();
        env.registry.push(holder("test:chosen"));
        let mut wire: HashSet<RecipePair> = HashSet::new();
        wire.insert(pair("test:zeta"));
        wire.insert(pair("test:alpha"));
        SPacketPlayerRecipeSync {
            recipe_list: Some(wire),
            selected: Some(id("test:chosen")),
        }
        .handle(&mut env);
        let list = env.player_data.get_recipes_list();
        let first = list.iter().next().unwrap();
        assert_eq!(first.resource_location, id("test:alpha"), "sorted, not wire order");
        assert_eq!(
            env.player_data.get_selected_recipe().map(|h| h.id.clone()),
            Some(id("test:chosen"))
        );
    }

    #[test]
    fn player_sync_with_no_list_clears_to_an_empty_set() {
        let mut env = TestClientEnv::new();
        SPacketPlayerRecipeSync { recipe_list: None, selected: None }.handle(&mut env);
        assert!(env.player_data.get_recipes_list().is_empty());
    }

    #[test]
    fn recipes_list_goes_to_the_widget_when_present() {
        let mut env = TestClientEnv::new();
        let mut wire: HashSet<RecipePair> = HashSet::new();
        wire.insert(pair("test:a"));
        SPacketRecipesList { recipe_list: Some(wire), selected: Some(id("test:a")) }.handle(&mut env);
        let widget = env.widget.as_ref().unwrap();
        assert_eq!(widget.sets.len(), 1);
        assert_eq!(widget.sets[0].0.iter().next().unwrap().resource_location, id("test:a"));
        assert_eq!(widget.sets[0].1, Some(id("test:a")));
        assert!(env.enqueued.is_empty());
    }

    #[test]
    fn recipes_list_is_enqueued_without_a_widget() {
        let mut env = TestClientEnv::new();
        env.widget = None;
        let mut wire: HashSet<RecipePair> = HashSet::new();
        wire.insert(pair("test:a"));
        SPacketRecipesList { recipe_list: Some(wire), selected: None }.handle(&mut env);
        assert_eq!(env.enqueued.len(), 1);
        assert_eq!(env.enqueued[0].0.len(), 1);
    }

    #[test]
    fn highlight_reaches_the_widget() {
        let mut env = TestClientEnv::new();
        SPacketHighlightRecipe { recipe: id("test:a") }.handle(&mut env);
        assert_eq!(env.widget.as_ref().unwrap().highlights, vec![id("test:a")]);
    }

    #[test]
    fn update_preview_only_fires_on_a_smithing_screen() {
        let mut env = TestClientEnv::new();
        SPacketUpdatePreview::INSTANCE.handle(&mut env);
        assert!(env.previews.is_empty(), "no smithing screen, no preview");
        env.smithing = true;
        SPacketUpdatePreview::INSTANCE.handle(&mut env);
        assert_eq!(env.previews.len(), 1);
        assert_eq!(env.previews[0].item, id("test:helm"), "slot 3's item");
    }

    #[test]
    fn packets_silently_ignore_a_missing_client_player() {
        let mut env = TestClientEnv::new();
        env.player = None;
        let mut wire: HashSet<RecipePair> = HashSet::new();
        wire.insert(pair("test:a"));
        SPacketPlayerRecipeSync { recipe_list: Some(wire.clone()), selected: None }.handle(&mut env);
        SPacketRecipesList { recipe_list: Some(wire), selected: None }.handle(&mut env);
        SPacketHighlightRecipe { recipe: id("test:a") }.handle(&mut env);
        assert!(env.widget.as_ref().unwrap().sets.is_empty());
        assert!(env.enqueued.is_empty());
        assert!(env.widget.as_ref().unwrap().highlights.is_empty());
    }
}
