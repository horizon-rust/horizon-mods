//! Tests for the 1:1 port. A process-wide lock serializes the tests that
//! touch the crate's global state (the favorites set, the client config, the
//! Create World toggle), mirroring the JVM's single-instance statics.

use horizon_mod_cherished_worlds::*;
use std::sync::{Mutex, OnceLock};

fn state_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn reset_favorites() {
    favorites_list::clear_for_tests();
    unsafe { std::env::remove_var("HORIZON_GAME_DIR"); }
}

// java: FavoritesList.save()/load() roundtrip through <modid>-favorites.dat
#[test]
fn favorites_save_load_roundtrip() {
    let _g = state_lock();
    reset_favorites();
    let dir = std::env::temp_dir().join(format!("cw-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    unsafe { std::env::set_var("HORIZON_GAME_DIR", &dir); }

    FavoritesList::add("New World");
    FavoritesList::add("Second World");
    FavoritesList::save();
    favorites_list::clear_for_tests();
    assert!(!FavoritesList::contains("New World"));

    FavoritesList::load();
    assert!(FavoritesList::contains("New World"));
    assert!(FavoritesList::contains("Second World"));
    assert!(!FavoritesList::contains("Third World"));

    // java: load() with a missing file clears and returns cleanly
    std::fs::remove_file(dir.join("cherishedworlds-favorites.dat")).unwrap();
    FavoritesList::load();
    assert!(favorites_list::snapshot_for_tests().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// java: contains / add / remove
#[test]
fn favorites_add_remove_contains() {
    let _g = state_lock();
    reset_favorites();
    FavoritesList::add("alpha");
    assert!(FavoritesList::contains("alpha"));
    FavoritesList::add("alpha");
    assert!(FavoritesList::contains("alpha"));
    FavoritesList::remove("alpha");
    assert!(!FavoritesList::contains("alpha"));
    FavoritesList::remove("missing");
    assert!(favorites_list::snapshot_for_tests().is_empty());
}

// java: getBackupStatus matrix over BackupType { ALL, FAVORITED, NONE }
#[test]
fn get_backup_status_matrix() {
    let _g = state_lock();
    reset_favorites();
    use config::{client, BackupType};
    use seams::{BackupStatus, LevelSummary};

    let old = LevelSummary { level_id: "old".into(), level_version: 100 };
    let newer = LevelSummary { level_id: "newer".into(), level_version: 10000 };

    // backupType ALL: any older level upgrades
    client().backup_world_type.set(BackupType::All);
    assert_eq!(
        mixin_hooks::get_backup_status(&old, BackupStatus::None),
        BackupStatus::UpgradeToSnapshot
    );
    assert_eq!(
        mixin_hooks::get_backup_status(&newer, BackupStatus::None),
        BackupStatus::None
    );
    assert_eq!(
        mixin_hooks::get_backup_status(&old, BackupStatus::UpgradeToSnapshot),
        BackupStatus::UpgradeToSnapshot
    );

    // backupType FAVORITED: only favorites
    client().backup_world_type.set(BackupType::Favorited);
    FavoritesList::add("fav");
    let fav = LevelSummary { level_id: "fav".into(), level_version: 100 };
    assert_eq!(
        mixin_hooks::get_backup_status(&fav, BackupStatus::None),
        BackupStatus::UpgradeToSnapshot
    );
    assert_eq!(
        mixin_hooks::get_backup_status(&old, BackupStatus::None),
        BackupStatus::None
    );

    // backupType NONE: never upgrades
    client().backup_world_type.set(BackupType::None);
    assert_eq!(
        mixin_hooks::get_backup_status(&fav, BackupStatus::None),
        BackupStatus::None
    );
    client().backup_world_type.set(BackupType::All);
}

// java: isNotValidSwap, including the IndexOutOfBoundsException path
#[test]
fn is_not_valid_swap() {
    let _g = state_lock();
    reset_favorites();
    let list = seams::ServerList {
        servers: vec![
            seams::ServerData { name: "fav".into(), ip: "1.2.3.4".into() },
            seams::ServerData { name: "plain".into(), ip: "5.6.7.8".into() },
        ],
    };
    FavoritesList::add("fav1.2.3.4");

    // favored vs unfavored is an invalid swap
    assert!(mixin_hooks::is_not_valid_swap(&list, 0, 1));
    assert!(mixin_hooks::is_not_valid_swap(&list, 1, 0));
    // same side is valid
    assert!(!mixin_hooks::is_not_valid_swap(&list, 0, 0));
    assert!(!mixin_hooks::is_not_valid_swap(&list, 1, 1));
    // out of range logs and refuses
    assert!(!mixin_hooks::is_not_valid_swap(&list, 0, 9));
    assert!(!mixin_hooks::is_not_valid_swap(&list, 9, 9));
}

// java: renameFavorite / editDeleteButton / canDelete
#[test]
fn rename_favorite_and_guards() {
    let _g = state_lock();
    reset_favorites();
    mixin_hooks::rename_favorite("old", "new");
    assert!(favorites_list::snapshot_for_tests().is_empty());

    FavoritesList::add("old");
    mixin_hooks::rename_favorite("old", "new");
    assert!(!FavoritesList::contains("old"));
    assert!(FavoritesList::contains("new"));

    let online = seams::ServerSelectionEntry::Online(seams::ServerData {
        name: "new".into(),
        ip: "x".into(),
    });
    // java: editDeleteButton -> !contains(name + ip)
    assert!(mixin_hooks::edit_delete_button(Some(&online), false));
    FavoritesList::add("newx");
    assert!(!mixin_hooks::edit_delete_button(Some(&online), false));

    let level = seams::LevelSummary { level_id: "fav".into(), level_version: 1 };
    FavoritesList::add("fav");
    assert!(!mixin_hooks::can_delete(&level));
    FavoritesList::remove("fav");
    assert!(mixin_hooks::can_delete(&level));
}

// java: updateOnlineServers / updateNetworkServers favorites-first ordering
#[test]
fn server_list_favorites_first() {
    let _g = state_lock();
    reset_favorites();
    let mut servers = seams::ServerList {
        servers: vec![
            seams::ServerData { name: "a".into(), ip: "1".into() },
            seams::ServerData { name: "b".into(), ip: "2".into() },
            seams::ServerData { name: "c".into(), ip: "3".into() },
        ],
    };
    FavoritesList::add("b2");
    let mut online = vec![
        seams::ServerSelectionEntry::Online(servers.servers[0].clone()),
        seams::ServerSelectionEntry::Online(servers.servers[1].clone()),
        seams::ServerSelectionEntry::Online(servers.servers[2].clone()),
    ];
    mixin_hooks::update_online_servers(&mut servers, &mut online);

    // favorites first, others keep their relative order
    let names: Vec<String> = online
        .iter()
        .map(|e| match e {
            seams::ServerSelectionEntry::Online(d) => d.name.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(names, vec!["b", "a", "c"]);
    // java: servers.replace(i, entry.getServerData())
    assert_eq!(servers.servers[0].name, "b");
    assert_eq!(servers.servers[1].name, "a");
    assert_eq!(servers.servers[2].name, "c");

    let mut network = vec![
        seams::ServerSelectionEntry::Network(seams::LanServer { address: "lan-a".into() }),
        seams::ServerSelectionEntry::Network(seams::LanServer { address: "lan-b".into() }),
    ];
    FavoritesList::add("lan-b");
    mixin_hooks::update_network_servers(&[], &mut network);
    let addrs: Vec<String> = network
        .iter()
        .map(|e| match e {
            seams::ServerSelectionEntry::Network(s) => s.address.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(addrs, vec!["lan-b", "lan-a"]);
}

// java: getLevelComparator favorites first, then l1.compareTo(l2)
#[test]
fn level_comparator_orders() {
    let _g = state_lock();
    reset_favorites();
    let fav = mixins::world_entry(Some(seams::LevelSummary { level_id: "fav".into(), level_version: 1 }));
    let aa = mixins::world_entry(Some(seams::LevelSummary { level_id: "aa".into(), level_version: 1 }));
    let zz = mixins::world_entry(Some(seams::LevelSummary { level_id: "zz".into(), level_version: 1 }));
    let none = mixins::world_entry(None);

    FavoritesList::add("zz");
    assert_eq!(mixin_hooks::level_comparator(&zz, &aa), std::cmp::Ordering::Less);
    assert_eq!(mixin_hooks::level_comparator(&aa, &zz), std::cmp::Ordering::Greater);
    assert_eq!(mixin_hooks::level_comparator(&aa, &aa), std::cmp::Ordering::Equal);
    // null summaries compare 0
    assert_eq!(mixin_hooks::level_comparator(&none, &aa), std::cmp::Ordering::Equal);

    // java: this.sort(CherishedWorldsMixinHooks.getLevelComparator())
    let mut list = seams::WorldSelectionList::default();
    list.children = vec![aa.clone(), zz.clone(), fav.clone()];
    mixins::MixinWorldSelectionList::cherishedworlds_sort_levels(&mut list);
    let ids: Vec<String> = list
        .children
        .iter()
        .filter_map(|e| e.summary.as_ref().map(|s| s.level_id.clone()))
        .collect();
    assert_eq!(ids, vec!["zz", "aa", "fav"]);
}

// java: getChildAt only answers on SelectWorld/JoinMultiplayer screens
#[test]
fn get_child_at_dispatch() {
    let _g = state_lock();
    reset_favorites();
    let screen = seams::Screen::new(seams::ScreenKind::SelectWorld);
    use widgets::abstract_favorites_list_widget::AbstractFavoritesListWidget;
    let widget = AbstractFavoritesListWidget::new(screen.clone(), None, vec![]);
    *widget.hovered_now.lock().unwrap() = true;

    assert!(mixins::MixinContainerEventHandler::cherishedworlds_get_child_at(
        &screen, false, Some(&widget), 0.0, 0.0
    ));

    // not hovering: the original (false) passes through
    *widget.hovered_now.lock().unwrap() = false;
    assert!(!mixins::MixinContainerEventHandler::cherishedworlds_get_child_at(
        &screen, false, Some(&widget), 0.0, 0.0
    ));

    // non-matching screen kinds return original untouched
    let other = seams::Screen::new(seams::ScreenKind::Other);
    assert!(!mixins::MixinContainerEventHandler::cherishedworlds_get_child_at(
        &other, false, Some(&widget), 0.0, 0.0
    ));
    assert!(mixins::MixinContainerEventHandler::cherishedworlds_get_child_at(
        &other, true, Some(&widget), 0.0, 0.0
    ));
}

// java: FavoriteCreateWorldWidget saveFavorite / toggle static
#[test]
fn create_world_favorite_gate() {
    let _g = state_lock();
    reset_favorites();
    widgets::favorite_create_world_widget::reset_for_tests();

    // not toggled: saveFavorite is a no-op and resets the flag
    widgets::favorite_create_world_widget::FavoriteCreateWorldWidget::save_favorite("world-a");
    assert!(favorites_list::snapshot_for_tests().is_empty());

    // java: onClick() toggles isFavorited; saveFavorite then stores the id
    let screen = seams::Screen::new(seams::ScreenKind::CreateWorld);
    let mut widget = widgets::favorite_create_world_widget::FavoriteCreateWorldWidget::for_screen(screen);
    assert!(!widgets::favorite_create_world_widget::is_favorited_for_tests());
    widget.on_click();
    assert!(widgets::favorite_create_world_widget::is_favorited_for_tests());
    widgets::favorite_create_world_widget::FavoriteCreateWorldWidget::save_favorite("world-a");
    assert!(FavoritesList::contains("world-a"));
    assert!(!widgets::favorite_create_world_widget::is_favorited_for_tests());
}

// java: FavoriteCreateWorldWidget positions itself next to the
// selectWorld.create button
#[test]
fn create_world_widget_positions_next_to_create_button() {
    let _g = state_lock();
    let mut screen = seams::Screen::new(seams::ScreenKind::CreateWorld);
    screen.host.children = vec![
        seams::ChildWidget::Button {
            base: seams::WidgetBase::new(300, 120, 150, 20),
            translation_key: Some("selectWorld.create".into()),
        },
        seams::ChildWidget::Listener,
    ];
    let widget = widgets::favorite_create_world_widget::FavoriteCreateWorldWidget::for_screen(screen);
    // java: x = getX() - 16, y = getY() + height/2 - 4
    assert_eq!(widget.inner.base.x, 300 - 16);
    assert_eq!(widget.inner.base.y, 120 + (20 / 2) - 4);
}

// java: areCoordinatesInIcon boundary math
#[test]
fn coordinates_in_icon() {
    assert!(widgets::abstract_favorites_widget::AbstractFavoritesWidget::are_coordinates_in_icon(
        10, 10, 9, 10.0, 10.0
    ));
    assert!(widgets::abstract_favorites_widget::AbstractFavoritesWidget::are_coordinates_in_icon(
        10, 10, 9, 18.0, 18.0
    ));
    assert!(!widgets::abstract_favorites_widget::AbstractFavoritesWidget::are_coordinates_in_icon(
        10, 10, 9, 19.0, 10.0
    ));
    assert!(!widgets::abstract_favorites_widget::AbstractFavoritesWidget::are_coordinates_in_icon(
        10, 10, 9, 10.0, 19.0
    ));
    assert!(!widgets::abstract_favorites_widget::AbstractFavoritesWidget::are_coordinates_in_icon(
        10, 10, 9, 9.9, 10.0
    ));
}

// java: the abstract list widget icon pass, tooltip key, and focus path
#[test]
fn list_widget_render_and_focus() {
    let _g = state_lock();
    reset_favorites();
    FavoritesList::add("world-a");
    use widgets::abstract_favorites_list_widget::{tooltip_key, AbstractFavoritesListWidget};

    let screen = seams::Screen::new(seams::ScreenKind::SelectWorld);
    let mut list = seams::WorldSelectionList::default();
    list.children = vec![
        mixins::world_entry(Some(seams::LevelSummary { level_id: "world-a".into(), level_version: 1 })),
        mixins::world_entry(Some(seams::LevelSummary { level_id: "world-b".into(), level_version: 1 })),
    ];
    list.selected_index = Some(0);
    list.top = 0;
    list.bottom = 100;
    let geometry = vec![
        seams::EntryGeometry { x: 40, y: 8, content_y: 8, content_bottom: 28, content_y_middle: 18, height: 20 },
        seams::EntryGeometry { x: 40, y: 28, content_y: 28, content_bottom: 48, content_y_middle: 38, height: 20 },
    ];
    let mut widget = AbstractFavoritesListWidget::new(screen.clone(), Some(list), geometry.clone());

    // icon pass: favored entry gets FAVORITED (or hovered variant), unfavored
    // gets EMPTY; icons sit at entry.x - 16
    let mut graphics = seams::GuiGraphicsExtractor { mouse_in_scissor: false, ..Default::default() };
    widget.extract_widget_render_state(&mut graphics, -100, -100);
    assert!(!widget.is_mouse_over());
    let fav_blits: Vec<&seams::BlitOp> = graphics
        .blits
        .iter()
        .filter(|b| b.x == 40 - 16)
        .collect();
    assert!(fav_blits.len() >= 2);
    assert!(fav_blits
        .iter()
        .any(|b| b.icon == seams::IconTexture::Favorited));
    assert!(fav_blits
        .iter()
        .any(|b| b.icon == seams::IconTexture::Empty));

    // hover inside the first icon sets hovered + tooltip
    let mut graphics = seams::GuiGraphicsExtractor { mouse_in_scissor: true, ..Default::default() };
    widget.extract_widget_render_state(&mut graphics, 24 + 2, geometry[0].content_y_middle - 4 + 2);
    assert!(widget.is_mouse_over());
    assert!(widget.get_hovered_entry().is_some());
    assert_eq!(widget.tooltip.tooltip.as_ref().unwrap().text.translation_key, tooltip_key(true));

    // java: onClick toggles the hovered entry only, saves, refilters,
    // returns the key (world-a was favorited, so the click removes it)
    let key = widget.on_click(widget.get_hovered_entry());
    assert_eq!(key, "world-a");
    assert!(!FavoritesList::contains("world-a"));
    assert!(!FavoritesList::contains("world-b"));

    // key press with selection acts like a click on the focused entry
    // (world-b, favorited again, so the press removes it too)
    FavoritesList::add("world-b");
    widget.focus_index = 1;
    widget.is_focused = true;
    assert!(widget.key_pressed(seams::KeyEvent { is_selection: true }));
    assert!(!FavoritesList::contains("world-b"));

    // java: nextFocusPath arrow down moves focus, leaf path returned
    // (2 children: focus 0 -> 1, then 1 + 1 = 2 is out of range -> None)
    widget.focus_index = 0;
    assert_eq!(widget.next_focus_path(seams::FocusNavigationEvent::Arrow {
        direction: seams::FocusDirection::Down,
    }), Some(seams::ComponentPath::Leaf));
    assert_eq!(widget.focus_index, 1);
    // out of range is None
    assert!(widget.next_focus_path(seams::FocusNavigationEvent::Arrow {
        direction: seams::FocusDirection::Down,
    }).is_none());
    // left/right never move
    assert!(widget.next_focus_path(seams::FocusNavigationEvent::Arrow {
        direction: seams::FocusDirection::Left,
    }).is_none());
    // tab clears focus
    widget.focus_index = 0;
    assert!(widget.next_focus_path(seams::FocusNavigationEvent::Tab).is_none());
    assert_eq!(widget.focus_index, -1);

    // java: narration builds TITLE + USAGE around the favorite state
    let mut out = seams::NarrationElementOutput::default();
    widget.update_widget_narration(Some(&mut out));
    assert_eq!(out.entries.len(), 2);
    assert!(out.entries[0].1.translation_key.starts_with("gui.narrate.cherishedworlds."));

    // java: getRectangle shifts 16px left of the list
    let rect = widget.get_rectangle();
    assert_eq!(rect.width, 16);
    assert_eq!(rect.left, -16);
}

// java: setFocused seeds focus on the selected entry, then applyFocus scrolls
#[test]
fn list_widget_focus_scrolls() {
    let _g = state_lock();
    use widgets::abstract_favorites_list_widget::AbstractFavoritesListWidget;

    let screen = seams::Screen::new(seams::ScreenKind::SelectWorld);
    let mut list = seams::WorldSelectionList::default();
    list.children = vec![
        mixins::world_entry(Some(seams::LevelSummary { level_id: "a".into(), level_version: 1 })),
        mixins::world_entry(Some(seams::LevelSummary { level_id: "b".into(), level_version: 1 })),
    ];
    list.selected_index = Some(1);
    list.top = 10;
    list.bottom = 50;
    list.scroll_amount = 100.0;
    let geometry = vec![
        seams::EntryGeometry { x: 40, y: 0, content_y: 0, content_bottom: 20, content_y_middle: 10, height: 20 },
        seams::EntryGeometry { x: 40, y: 60, content_y: 60, content_bottom: 80, content_y_middle: 70, height: 20 },
    ];
    let mut widget = AbstractFavoritesListWidget::new(screen, Some(list), geometry);

    // set_focused seeds focus to the selected entry (index 1) and, through
    // setFocusIndex, runs applyFocus: entry bottom 80 > 50, bottomDelta =
    // 50 - 60 - 20 - 2 = -32, scrollAmount -= bottomDelta -> 132
    widget.set_focused(true);
    assert_eq!(widget.focus_index, 1);
    assert!(widget.is_focused());
    assert!((widget.world_list.as_ref().unwrap().scroll_amount() - 132.0).abs() < 1e-9);

    // set_focused(false) clears
    widget.set_focused(false);
    assert!(!widget.is_focused());
}

// java: FavoriteServersWidget get_key / updateList / delete-button gating
#[test]
fn servers_widget_key_and_delete_gate() {
    let _g = state_lock();
    reset_favorites();
    use widgets::favorite_servers_widget::FavoriteServersWidget;

    let mut screen = seams::Screen::new(seams::ScreenKind::JoinMultiplayer);
    screen.host.servers = seams::ServerList {
        servers: vec![
            seams::ServerData { name: "hyp".into(), ip: ".net".into() },
            seams::ServerData { name: "lan".into(), ip: "9".into() },
        ],
    };
    let mut list = seams::ServerSelectionList::default();
    list.online_servers = vec![
        seams::ServerSelectionEntry::Online(seams::ServerData { name: "hyp".into(), ip: ".net".into() }),
        seams::ServerSelectionEntry::Network(seams::LanServer { address: "lan9".into() }),
    ];
    list.selected = Some(seams::ServerSelectionEntry::Online(seams::ServerData {
        name: "hyp".into(),
        ip: ".net".into(),
    }));
    let widget = FavoriteServersWidget::for_screen(screen.clone(), Some(list), vec![]);

    // java: key = serverData.name + serverData.ip, or the LanServer address
    assert_eq!(widget.get_key(&seams::ServerSelectionEntry::Online(seams::ServerData {
        name: "hyp".into(),
        ip: ".net".into(),
    })), "hyp.net");
    assert_eq!(
        widget.get_key(&seams::ServerSelectionEntry::Network(seams::LanServer { address: "lan9".into() })),
        "lan9"
    );
    assert_eq!(widget.get_key(&seams::ServerSelectionEntry::Other), "");
    // java: getNarratableId = name or address
    assert_eq!(
        widget.get_narratable_id(&seams::ServerSelectionEntry::Online(seams::ServerData {
            name: "hyp".into(),
            ip: ".net".into(),
        })),
        "hyp"
    );
    assert_eq!(
        widget.get_narratable_id(&seams::ServerSelectionEntry::Network(seams::LanServer { address: "lan9".into() })),
        "lan9"
    );
}

// java: FavoriteWorldsWidget get_key and the refilter seam
#[test]
fn worlds_widget_key_and_refilter() {
    let _g = state_lock();
    use widgets::favorite_worlds_widget::FavoriteWorldsWidget;

    let mut screen = seams::Screen::new(seams::ScreenKind::SelectWorld);
    screen.host.search_box_value = "surv".into();
    let mut list = seams::WorldSelectionList::default();
    list.currently_displayed_levels = Some(vec![seams::LevelSummary {
        level_id: "survival".into(),
        level_version: 1,
    }]);
    let mut widget = FavoriteWorldsWidget::for_screen(screen.clone(), Some(list), vec![]);

    assert_eq!(
        widget.get_key(&mixins::world_entry(Some(seams::LevelSummary {
            level_id: "survival".into(),
            level_version: 1,
        }))),
        "survival"
    );
    assert_eq!(widget.get_key(&mixins::world_entry(None)), "");

    // java: updateList feeds the search box value into callFillLevels
    widget.widget.search_filter = Some(screen.host.search_box_value.clone());
    widget.widget.update_list();
    assert_eq!(
        widget.get_list().unwrap().fill_levels_filter.as_deref(),
        Some("surv")
    );
}

// java: mixin injection call sites behave like their Java counterparts
#[test]
fn mixin_call_sites() {
    let _g = state_lock();
    reset_favorites();
    FavoritesList::add("hyp.net");

    // MixinJoinMultiplayerScreen.cherishedworlds$initButtons
    let list = seams::ServerSelectionList {
        selected: Some(seams::ServerSelectionEntry::Online(seams::ServerData {
            name: "hyp".into(),
            ip: ".net".into(),
        })),
        ..Default::default()
    };
    assert!(!mixins::MixinJoinMultiplayerScreen::cherishedworlds_init_buttons(&list, true));

    // MixinServerSelectionListEntry.cherishedworld$swapServers
    let servers = seams::ServerList {
        servers: vec![
            seams::ServerData { name: "hyp".into(), ip: ".net".into() },
            seams::ServerData { name: "z".into(), ip: "2".into() },
        ],
    };
    assert!(mixins::MixinServerSelectionListEntry::cherishedworld_swap_servers(&servers, 0, 1));
    assert!(!mixins::MixinServerSelectionListEntry::cherishedworld_swap_servers(&servers, 1, 1));

    // MixinManageServerScreen.cherishedworlds$onAdd
    let server_data = seams::ServerData { name: "hyp".into(), ip: ".net".into() };
    mixins::MixinManageServerScreen::cherishedworlds_on_add(&server_data, "newname", "newip");
    assert!(!FavoritesList::contains("hyp.net"));
    assert!(FavoritesList::contains("newnamenewip"));

    // MixinLevelSummary.cherishedworlds$canDelete (re-add: onAdd renamed it)
    let level = seams::LevelSummary { level_id: "hyp.net".into(), level_version: 1 };
    FavoritesList::add("hyp.net");
    assert!(!mixins::MixinLevelSummary::cherishedworlds_can_delete(&level));

    // MixinWorldOpenFlows wrap + inject: an older level upgrades under
    // BackupType ALL, a current one keeps the original status
    let status = mixins::MixinWorldOpenFlows::cherishedworlds_open_world_check_version_compatibility(
        &level,
        seams::BackupStatus::None,
    );
    assert_eq!(status, seams::BackupStatus::UpgradeToSnapshot);
    let fresh = seams::LevelSummary {
        level_id: "fresh".into(),
        level_version: seams::current_data_version(),
    };
    let status = mixins::MixinWorldOpenFlows::cherishedworlds_open_world_check_version_compatibility(
        &fresh,
        seams::BackupStatus::None,
    );
    assert_eq!(status, seams::BackupStatus::None);
    mixins::MixinWorldOpenFlows::cherishedworlds_create_level_from_existing_settings("hyp.net");
    // onCreateNewWorld -> saveFavorite, flag off = no-op

    // AccessorWorldSelectionList invoker
    let mut wl = seams::WorldSelectionList::default();
    mixins::AccessorWorldSelectionList::call_fill_levels(&mut wl, "filter", &[]);
    assert_eq!(wl.fill_levels_filter.as_deref(), Some("filter"));
}

// java: ScreenEventHooks.addFavoritesWidget dispatch by screen kind
#[test]
fn screen_event_hooks_dispatch() {
    let _g = state_lock();
    for (kind, expected) in [
        (seams::ScreenKind::CreateWorld, "create"),
        (seams::ScreenKind::JoinMultiplayer, "servers"),
        (seams::ScreenKind::SelectWorld, "worlds"),
        (seams::ScreenKind::Other, "none"),
    ] {
        let screen = seams::Screen::new(kind);
        let decision = screen_event_hooks::ScreenEventHooks::add_favorites_widget(&screen);
        let got = match decision {
            screen_event_hooks::WidgetDecision::None => "none",
            screen_event_hooks::WidgetDecision::CreateWorld(_) => "create",
            screen_event_hooks::WidgetDecision::JoinMultiplayer(_) => "servers",
            screen_event_hooks::WidgetDecision::SelectWorld(_) => "worlds",
        };
        assert_eq!(got, expected);
    }
}

// java: Services platform helper + config prefix
#[test]
fn services_and_config() {
    let _g = state_lock();
    let platform = services::Services::platform();
    assert!(!services::IPlatformHelper::is_mod_loaded(&platform, "anything"));
    // java: getGamePath -> the engine's game dir seam (HORIZON_GAME_DIR)
    let dir = std::env::temp_dir().join("cw-services-test");
    std::fs::create_dir_all(&dir).unwrap();
    unsafe { std::env::set_var("HORIZON_GAME_DIR", &dir); }
    assert_eq!(services::IPlatformHelper::get_game_path(&platform), dir);
    unsafe { std::env::remove_var("HORIZON_GAME_DIR"); }
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(config::config_prefix(), "gui.cherishedworlds.config.");
    let spec = config::client_spec();
    assert_eq!(spec.backup_world_type.default, config::BackupType::All);
    assert_eq!(
        spec.backup_world_type.translation,
        "gui.cherishedworlds.config.backupWorldType"
    );
    common_mod::CherishedWorldsCommonMod::setup_config();
    common_mod::CherishedWorldsCommonMod::setup();
}
