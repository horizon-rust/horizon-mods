//! 1:1 port of mixin/CherishedWorldsMixinHooks.

use crate::config::{client, BackupType};
use crate::constants::{log, MOD_ID};
use crate::favorites_list::FavoritesList;
use crate::seams::{
    current_data_version, BackupStatus, LanServer, LevelSummary, ScreenKind, ServerList,
    ServerSelectionEntry, WorldSelectionEntry,
};
use crate::widgets::abstract_favorites_list_widget::AbstractFavoritesListWidget;
use std::cmp::Ordering;

// java: public static LevelSummary.BackupStatus getBackupStatus(LevelSummary
//           levelSummary, LevelSummary.BackupStatus original)
pub fn get_backup_status(level_summary: &LevelSummary, original: BackupStatus) -> BackupStatus {
    // java: CherishedWorldsConfig.BackupType backupType =
    //           CherishedWorldsConfig.CLIENT.backupWorldType.get();
    let backup_type = client().backup_world_type.get();

    if original == BackupStatus::None && backup_type != BackupType::None {
        // java: if (backupType == BackupType.ALL || FavoritesList.contains(
        //           levelSummary.getLevelId()))
        if backup_type == BackupType::All || FavoritesList::contains(&level_summary.level_id) {
            // java: int levelVersion = levelSummary.levelVersion()
            //                            .minecraftVersion().version();
            //       int currentVersion = SharedConstants.getCurrentVersion()
            //                                .dataVersion().version();
            let level_version = level_summary.level_version;
            let current_version = current_data_version();

            if level_version < current_version {
                // java: return LevelSummary.BackupStatus.UPGRADE_TO_SNAPSHOT;
                return BackupStatus::UpgradeToSnapshot;
            }
        }
    }
    original
}

// java: public static boolean isNotValidSwap(ServerList serverList, int pos1,
//           int pos2)
pub fn is_not_valid_swap(server_list: &ServerList, pos1: i32, pos2: i32) -> bool {
    // java: int offset = 0;
    let mut offset = 0;

    if pos2 as usize >= server_list.size() {
        offset = pos2 - server_list.size() as i32 + 1;
    } else if pos1 as usize >= server_list.size() {
        offset = pos1 - server_list.size() as i32 + 1;
    }

    // java: try { ServerData data1 = serverList.get(pos1 - offset); ...
    //       } catch (IndexOutOfBoundsException e) {
    //           LOG.error("Error trying to swap servers!", e); }
    let index1 = (pos1 - offset) as usize;
    let index2 = (pos2 - offset) as usize;
    if index1 >= server_list.size() || index2 >= server_list.size() {
        log().error("Error trying to swap servers!");
        return false;
    }
    let data1 = server_list.get(index1).unwrap();
    let data2 = server_list.get(index2).unwrap();
    // java: boolean isFavored1 = FavoritesList.contains(data1.name + data1.ip);
    let is_favored1 = FavoritesList::contains(&data1.favorite_key());
    let is_favored2 = FavoritesList::contains(&data2.favorite_key());
    (is_favored1 && !is_favored2) || (!is_favored1 && is_favored2)
}

// java: public static void renameFavorite(String prevName, String newName)
pub fn rename_favorite(prev_name: &str, new_name: &str) {
    if FavoritesList::contains(prev_name) {
        FavoritesList::remove(prev_name);
        FavoritesList::add(new_name);
    }
}

// java: public static boolean editDeleteButton(ServerSelectionList.Entry entry,
//           boolean flag)
pub fn edit_delete_button(entry: Option<&ServerSelectionEntry>, flag: bool) -> bool {
    match entry {
        // java: if (entry instanceof ServerSelectionList.OnlineServerEntry) {
        //           ServerData data = entry.getServerData();
        //           return !FavoritesList.contains(data.name + data.ip); }
        Some(ServerSelectionEntry::Online(data)) => {
            !FavoritesList::contains(&data.favorite_key())
        }
        _ => flag,
    }
}

// java: public static void updateOnlineServers(ServerList servers,
//           List<ServerSelectionList.OnlineServerEntry> onlineServers)
pub fn update_online_servers(servers: &mut ServerList, online_servers: &mut Vec<ServerSelectionEntry>) {
    // java: List<OnlineServerEntry> favorites = new ArrayList<>();
    //       List<OnlineServerEntry> others = new ArrayList<>();
    let mut favorites: Vec<ServerSelectionEntry> = Vec::new();
    let mut others: Vec<ServerSelectionEntry> = Vec::new();

    // java: for (OnlineServerEntry onlineServer : onlineServers) { ...
    for online_server in online_servers.iter() {
        let data = match online_server {
            ServerSelectionEntry::Online(data) => data,
            _ => continue,
        };

        if FavoritesList::contains(&data.favorite_key()) {
            favorites.push(online_server.clone());
        } else {
            others.push(online_server.clone());
        }
    }
    // java: onlineServers.clear();
    //       onlineServers.addAll(favorites);
    //       onlineServers.addAll(others);
    online_servers.clear();
    online_servers.extend(favorites);
    online_servers.extend(others);

    // java: for (int i = 0; i < onlineServers.size(); i++) {
    //           servers.replace(i, onlineServers.get(i).getServerData()); }
    for (i, online_server) in online_servers.iter().enumerate() {
        if let ServerSelectionEntry::Online(data) = online_server {
            servers.replace(i, data.clone());
        }
    }
}

// java: public static void updateNetworkServers(List<LanServer> servers,
//           List<ServerSelectionList.NetworkServerEntry> onlineServers)
pub fn update_network_servers(_servers: &[LanServer], online_servers: &mut Vec<ServerSelectionEntry>) {
    let mut favorites: Vec<ServerSelectionEntry> = Vec::new();
    let mut others: Vec<ServerSelectionEntry> = Vec::new();

    // java: LanServer data = ((AccessorNetworkServerEntry) onlineServer)
    //                            .getServerData();
    for online_server in online_servers.iter() {
        let data = match online_server {
            ServerSelectionEntry::Network(data) => data,
            _ => continue,
        };

        // java: if (FavoritesList.contains(data.getAddress()))
        if FavoritesList::contains(&data.address) {
            favorites.push(online_server.clone());
        } else {
            others.push(online_server.clone());
        }
    }
    online_servers.clear();
    online_servers.extend(favorites);
    online_servers.extend(others);
}

// java: public static Comparator<WorldSelectionList.Entry> getLevelComparator()
pub fn level_comparator(
    o1: &WorldSelectionEntry,
    o2: &WorldSelectionEntry,
) -> Ordering {
    let l1 = o1.summary.as_ref();
    let l2 = o2.summary.as_ref();

    // java: if (l1 != null && l2 != null) { ... }
    if let (Some(l1), Some(l2)) = (l1, l2) {
        let is_favorite1 = FavoritesList::contains(&l1.level_id);
        let is_favorite2 = FavoritesList::contains(&l2.level_id);

        if is_favorite1 && !is_favorite2 {
            return Ordering::Less;
        } else if !is_favorite1 && is_favorite2 {
            return Ordering::Greater;
        }
        // java: return l1.compareTo(l2);
        return l1.level_id.cmp(&l2.level_id);
    }
    Ordering::Equal
}

// java: public static Optional<GuiEventListener> getChildAt(
//           ContainerEventHandler eventHandler,
//           Optional<GuiEventListener> original, double x, double y)
pub fn get_child_at(
    event_handler_screen_kind: ScreenKind,
    original: bool,
    widget_over: Option<&AbstractFavoritesListWidget>,
    _x: f64,
    _y: f64,
) -> bool {
    // java: if (eventHandler instanceof SelectWorldScreen || eventHandler
    //           instanceof JoinMultiplayerScreen) {
    if event_handler_screen_kind == ScreenKind::SelectWorld
        || event_handler_screen_kind == ScreenKind::JoinMultiplayer
    {
        // java: for (GuiEventListener child : eventHandler.children()) {
        //           if (child instanceof AbstractFavoritesListWidget<?, ?> widget) {
        //               if (widget.isMouseOver(x, y)) { return Optional.of(child);
        //               } else { return original; } } }
        if let Some(widget) = widget_over {
            if widget.is_mouse_over() {
                return true;
            }
            return original;
        }
    }
    original
}

// java: public static boolean canDelete(LevelSummary levelSummary)
pub fn can_delete(level_summary: &LevelSummary) -> bool {
    !FavoritesList::contains(&level_summary.level_id)
}

// Test-only re-export so tests can assert tooltip keys the widget builds.
pub fn tooltip_key(is_favorite: bool) -> String {
    format!("selectWorld.{MOD_ID}.{}", if is_favorite { "unfavorite" } else { "favorite" })
}
