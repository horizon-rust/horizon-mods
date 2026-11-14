//! cherished_worlds launcher runtime splice (1:1 port of the LGPL-3.0
//! Cherished Worlds mod by Illusive Soulworks).
//!
//! This file is the Horizon native-mod wiring the launcher compiles into its
//! client runtime: it drives the 1:1 modules (FavoritesList, Config,
//! MixinHooks, the favorite widgets) from the launcher's own world-list and
//! screen events. Every call below forwards to the exact upstream hook or
//! static the Java mixin/hook pair would have hit, in upstream order.

use crate::config::{self, BackupType};
use crate::favorites_list;
use crate::mixin_hooks;
use crate::mixins;
use crate::seams;

/// Launcher-side wiring for the 1:1 port: load, hook, and serve the favorite
/// set. Upstream has no equivalent class (it reaches the game through mixins);
/// this splice is only the host binding and adds no policy of its own.
#[derive(Clone, Debug, Default)]
pub struct CherishedWorldsRuntime {
    pub search_filter: Option<String>,
}

impl CherishedWorldsRuntime {
    /// java: CherishedWorldsCommonMod.init() - load() on mod init.
    pub fn init(&self) {
        favorites_list::load();
    }

    /// java: MixinHooks.getBackupStatus(levelSummary, original)
    pub fn get_backup_status(&self, level_summary: &seams::LevelSummary, original: seams::BackupStatus) -> seams::BackupStatus {
        mixin_hooks::get_backup_status(level_summary, original)
    }

    /// java: MixinHooks.isNotValidSwap(serverList, pos1, pos2)
    pub fn is_not_valid_swap(&self, server_list: &seams::ServerList, pos1: i32, pos2: i32) -> bool {
        mixin_hooks::is_not_valid_swap(server_list, pos1, pos2)
    }

    /// java: MixinHooks.renameFavorite(prevName, newName)
    pub fn rename_favorite(&self, prev_name: &str, new_name: &str) {
        mixin_hooks::rename_favorite(prev_name, new_name);
    }

    /// java: MixinHooks.cherishedworlds$fillFavoritesList / widget tree entry:
    /// the world list renders through FavoriteWorldsWidget (favorites first).
    pub fn make_worlds_widget(&self, currently_displayed_levels: Option<Vec<seams::LevelSummary>>) -> mixins::FavoriteWorldsWidget {
        mixins::FavoriteWorldsWidget::new(self.search_filter.clone(), currently_displayed_levels)
    }

    /// java: MixinHooks.cherishedworlds$fillFavoritesServersList: the server
    /// list renders through FavoriteServersWidget (favorites first, swap
    /// guard active).
    pub fn make_servers_widget(&self) -> mixins::FavoriteServersWidget {
        mixins::FavoriteServersWidget::new(self.search_filter.clone())
    }

    /// java: Config.CLIENT.backupWorldType.get(), as the launcher reads it.
    pub fn backup_world_type(&self) -> BackupType {
        config::client().backup_world_type.get()
    }

    /// The favorite set, as the launcher persists it with its world index.
    pub fn favorites_snapshot(&self) -> Vec<String> {
        favorites_list::snapshot_for_tests()
    }

    /// java: FavoritesList.contains(fileName)
    pub fn is_favorite(&self, file_name: &str) -> bool {
        favorites_list::contains(file_name)
    }
}
