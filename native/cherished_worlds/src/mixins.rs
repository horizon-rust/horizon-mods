//! 1:1 port of the mixin layer (mixin/core/*). Each upstream @Mixin class is
//! a module here: the accessor/invoker seams the engine binds, plus the
//! injection call sites that forward to mixin_hooks. Injection points are
//! documented per function, verbatim from the Java annotations.

use crate::mixin_hooks as hooks;
use crate::seams::{
    LanServer, LevelSummary, Screen, ServerData, ServerList, ServerSelectionEntry,
    ServerSelectionList, WorldSelectionEntry, WorldSelectionList,
};

// java: @Mixin(JoinMultiplayerScreen.class) AccessorJoinMultiplayerScreen
pub struct AccessorJoinMultiplayerScreen;

impl AccessorJoinMultiplayerScreen {
    // java: @Accessor(value = "serverSelectionList")
    //       ServerSelectionList getSelectionList();
    pub fn get_selection_list(screen: &Screen) -> Option<ServerSelectionList> {
        screen.host.server_selection_list.clone()
    }

    // java: @Accessor Button getDeleteButton(); (only its active flag is read)
    pub fn get_delete_button_active(screen: &Screen) -> bool {
        screen.host.delete_button_active
    }
}

// java: @Mixin(ServerSelectionList.NetworkServerEntry.class)
//       AccessorNetworkServerEntry
pub struct AccessorNetworkServerEntry;

impl AccessorNetworkServerEntry {
    // java: @Accessor LanServer getServerData();
    pub fn get_server_data(entry: &ServerSelectionEntry) -> Option<LanServer> {
        match entry {
            ServerSelectionEntry::Network(server) => Some(server.clone()),
            _ => None,
        }
    }
}

// java: @Mixin(WorldSelectionList.class) AccessorWorldSelectionList
pub struct AccessorWorldSelectionList;

impl AccessorWorldSelectionList {
    // java: @Accessor List<LevelSummary> getCurrentlyDisplayedLevels();
    pub fn get_currently_displayed_levels(list: &WorldSelectionList) -> Option<Vec<LevelSummary>> {
        list.currently_displayed_levels.clone()
    }

    // java: @Invoker void callFillLevels(String filter, List<LevelSummary>
    //           levelSummaries);
    pub fn call_fill_levels(list: &mut WorldSelectionList, filter: &str, _level_summaries: &[LevelSummary]) {
        list.fill_levels_filter = Some(filter.to_string());
    }
}

// java: @Mixin(SelectWorldScreen.class) AccessorWorldSelectionScreen
pub struct AccessorWorldSelectionScreen;

impl AccessorWorldSelectionScreen {
    // java: @Accessor EditBox getSearchBox();
    pub fn get_search_box(screen: &Screen) -> Option<String> {
        Some(screen.host.search_box_value.clone())
    }

    // java: @Accessor WorldSelectionList getList();
    pub fn get_list(screen: &Screen) -> Option<WorldSelectionList> {
        screen.host.world_list.clone()
    }
}

// java: @Mixin(JoinMultiplayerScreen.class) MixinJoinMultiplayerScreen
pub struct MixinJoinMultiplayerScreen;

impl MixinJoinMultiplayerScreen {
    // java: @Inject(at = @At("TAIL"), method = "onSelectedChange")
    //       private void cherishedworlds$initButtons(CallbackInfo ci) {
    //           this.deleteButton.active = CherishedWorldsMixinHooks
    //               .editDeleteButton(this.serverSelectionList.getSelected(),
    //                                this.deleteButton.active); }
    pub fn cherishedworlds_init_buttons(
        server_selection_list: &ServerSelectionList,
        delete_button_active: bool,
    ) -> bool {
        hooks::edit_delete_button(server_selection_list.get_selected().as_ref(), delete_button_active)
    }
}

// java: @Mixin(ServerSelectionList.class) MixinServerSelectionList
pub struct MixinServerSelectionList;

impl MixinServerSelectionList {
    // java: @Inject(at = @At(value = "INVOKE", target =
    //           ".../ServerSelectionList.refreshEntries()V"), method =
    //           "updateOnlineServers")
    //       private void cherishedworlds$updateOnlineServers(ServerList
    //           serverList, CallbackInfo ci) {
    //           CherishedWorldsMixinHooks.updateOnlineServers(serverList,
    //               this.onlineServers); }
    pub fn cherishedworlds_update_online_servers(
        server_list: &mut ServerList,
        online_servers: &mut Vec<ServerSelectionEntry>,
    ) {
        hooks::update_online_servers(server_list, online_servers);
    }

    // java: @Inject(at = @At(value = "INVOKE", target =
    //           ".../ServerSelectionList.refreshEntries()V"), method =
    //           "updateNetworkServers")
    //       private void cherishedworlds$updateNetworkServers(List<LanServer>
    //           serverList, CallbackInfo ci) {
    //           CherishedWorldsMixinHooks.updateNetworkServers(serverList,
    //               this.networkServers); }
    pub fn cherishedworlds_update_network_servers(
        server_list: &[LanServer],
        network_servers: &mut Vec<ServerSelectionEntry>,
    ) {
        hooks::update_network_servers(server_list, network_servers);
    }
}

// java: @Mixin(ServerSelectionList.OnlineServerEntry.class)
//       MixinServerSelectionListEntry
pub struct MixinServerSelectionListEntry;

impl MixinServerSelectionListEntry {
    // java: @Inject(at = @At("HEAD"), method = "swap", cancellable = true)
    //       private void cherishedworld$swapServers(int pos1, int pos2,
    //           CallbackInfo ci) { if (CherishedWorldsMixinHooks.isNotValidSwap(
    //           this.screen.getServers(), pos1, pos2)) { ci.cancel(); } }
    // Returns whether the swap must be cancelled.
    pub fn cherishedworld_swap_servers(
        servers: &ServerList,
        pos1: i32,
        pos2: i32,
    ) -> bool {
        hooks::is_not_valid_swap(servers, pos1, pos2)
    }
}

// java: @Mixin(LevelSummary.class) MixinLevelSummary
pub struct MixinLevelSummary;

impl MixinLevelSummary {
    // java: @Inject(at = @At("RETURN"), method = "canDelete", cancellable = true)
    //       private void cherishedworlds$canDelete(CallbackInfoReturnable<Boolean>
    //       cir) { if (!CherishedWorldsMixinHooks.canDelete((LevelSummary)
    //       (Object) this)) { cir.setReturnValue(false); } }
    // Returns the effective canDelete value.
    pub fn cherishedworlds_can_delete(level_summary: &LevelSummary) -> bool {
        hooks::can_delete(level_summary)
    }
}

// java: @Mixin(ContainerEventHandler.class) MixinContainerEventHandler
pub struct MixinContainerEventHandler;

impl MixinContainerEventHandler {
    // java: @ModifyReturnValue(at = @At("RETURN"), method =
    //           "getChildAt(DD)Ljava/util/Optional;")
    //       private Optional<GuiEventListener> cherishedworlds$getChildAt(
    //           Optional<GuiEventListener> original, double x, double y) {
    //           return CherishedWorldsMixinHooks.getChildAt(
    //               (ContainerEventHandler) this, original, x, y); }
    pub fn cherishedworlds_get_child_at(
        screen: &Screen,
        original: bool,
        widget_over: Option<&crate::widgets::abstract_favorites_list_widget::AbstractFavoritesListWidget>,
        x: f64,
        y: f64,
    ) -> bool {
        hooks::get_child_at(screen.kind(), original, widget_over, x, y)
    }
}

// java: @Mixin(WorldOpenFlows.class) MixinWorldOpenFlows
pub struct MixinWorldOpenFlows;

impl MixinWorldOpenFlows {
    // java: @WrapOperation(at = @At(value = "INVOKE", target =
    //           ".../LevelSummary.backupStatus ()..."), method =
    //           "openWorldCheckVersionCompatibility")
    //       private LevelSummary.BackupStatus
    //           cherishedworlds$openWorldCheckVersionCompatibility(
    //           LevelSummary instance, Operation<LevelSummary.BackupStatus>
    //           original) { return CherishedWorldsMixinHooks.getBackupStatus(
    //           instance, original.call(instance)); }
    pub fn cherishedworlds_open_world_check_version_compatibility(
        instance: &LevelSummary,
        original: crate::seams::BackupStatus,
    ) -> crate::seams::BackupStatus {
        hooks::get_backup_status(instance, original)
    }

    // java: @Inject(at = @At("HEAD"), method =
    //           "createLevelFromExistingSettings")
    //       private void cherishedworlds$createLevelFromExistingSettings(...)
    //       { ScreenEventHooks.onCreateNewWorld(
    //           levelStorageAccess.getLevelId()); }
    pub fn cherishedworlds_create_level_from_existing_settings(level_id: &str) {
        crate::screen_event_hooks::ScreenEventHooks::on_create_new_world(level_id);
    }
}

// java: @Mixin(WorldSelectionList.class) MixinWorldSelectionList
pub struct MixinWorldSelectionList;

impl MixinWorldSelectionList {
    // java: @Inject(at = @At(value = "INVOKE", target =
    //           ".../WorldSelectionList.setSelected(...)V"), method =
    //           "fillLevels(Ljava/lang/String;Ljava/util/List;)V")
    //       private void cherishedworlds$sortLevels(String filter,
    //           List<LevelSummary> levels, CallbackInfo ci) {
    //           this.sort(CherishedWorldsMixinHooks.getLevelComparator()); }
    pub fn cherishedworlds_sort_levels(list: &mut WorldSelectionList) {
        list.sort(&hooks::level_comparator);
    }
}

// java: @Mixin(ManageServerScreen.class) MixinManageServerScreen
pub struct MixinManageServerScreen;

impl MixinManageServerScreen {
    // java: @Inject(at = @At("HEAD"), method = "onAdd")
    //       private void cherishedworlds$onAdd(CallbackInfo ci) {
    //           CherishedWorldsMixinHooks.renameFavorite(
    //               this.serverData.name + this.serverData.ip,
    //               this.nameEdit.getValue() + this.ipEdit.getValue()); }
    pub fn cherishedworlds_on_add(
        server_data: &ServerData,
        name_edit_value: &str,
        ip_edit_value: &str,
    ) {
        hooks::rename_favorite(
            &server_data.favorite_key(),
            &format!("{}{}", name_edit_value, ip_edit_value),
        );
    }
}

// test seam: expose a world entry builder for tests.
pub fn world_entry(summary: Option<LevelSummary>) -> WorldSelectionEntry {
    WorldSelectionEntry { summary }
}
