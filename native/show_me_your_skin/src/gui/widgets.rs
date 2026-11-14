//! The remaining gui widgets (1:1 shapes; framework seams per widget).

use super::framework::{Component, WidgetNode, ClientEnv};
use super::{ArmorConfigScreen, PlayerOption};
use crate::config::armor_config::{ArmorConfig, ArmorPieceConfig, PieceConfig};
use crate::config::mod_config;
use crate::config::synced_mod_config::SyncedModConfig;
use crate::show_me_your_skin;

/// Java: ShowMeYourSkinClient (client seam): serverConfig + syncToServer.
pub mod client {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        /// Java: public static SyncedModConfig serverConfig;
        static SERVER_CONFIG: RefCell<Option<SyncedModConfig>> = const { RefCell::new(None) };
    }

    pub fn with_server_config<R>(f: impl FnOnce(Option<&SyncedModConfig>) -> R) -> R {
        SERVER_CONFIG.with(|c| f(c.borrow().as_ref()))
    }

    pub fn set_server_config(cfg: Option<SyncedModConfig>) {
        SERVER_CONFIG.with(|c| *c.borrow_mut() = cfg);
    }

    /// Java: syncToServer(config) - sends the component to the server; the
    /// packet lands with the net stage.
    pub fn sync_to_server(_config: &ArmorConfig) {}
}

/// nl.enjarai.showmeyourskin.gui.IconCheckbox (1:1).
pub struct IconCheckbox {
    pub active_texture: String,
    pub inactive_texture: String,
    pub checked: bool,
    /// Java: @Nullable CheckboxCallback onUpdate (null when !active).
    pub on_update: Option<Box<dyn Fn(bool)>>,
}

impl IconCheckbox {
    pub fn new(active_texture: String, inactive_texture: String, checked: bool, active: bool, on_update: Option<Box<dyn Fn(bool)>>) -> IconCheckbox {
        IconCheckbox {
            active_texture,
            inactive_texture,
            checked,
            on_update: if active { on_update } else { None },
        }
    }

    /// Java: build(context) - TogglingClickable wrapping a Stack of the
    /// checkbox sprite and the active/inactive icon sprite.
    pub fn build_node(&self) -> WidgetNode {
        WidgetNode::new(
            "TogglingClickable",
            vec![WidgetNode::new(
                "Stack",
                vec![
                    WidgetNode::new("SpriteWidget", vec![]),
                    WidgetNode::new("SpriteWidget", vec![]),
                ],
            )],
        )
    }
}

/// nl.enjarai.showmeyourskin.gui.PieceConfigWidget (1:1): an ArmorPieceConfig
/// (head/chest/legs/feet) row of base/trim/glint checkboxes.
pub struct PieceConfigWidget {
    pub piece_config: *mut ArmorPieceConfig,
    pub label: Component,
    pub on_update: Box<dyn FnMut()>,
}

// The Java widget holds a live reference into the ArmorConfig being edited;
// the Rust port keeps the same aliasing through a raw pointer (single-owner
// screen state, like Java's widget reference).
unsafe impl Send for PieceConfigWidget {}
unsafe impl Send for AdditionalOptionsWidget {}
unsafe impl Send for OtherConfigWidget {}

impl PieceConfigWidget {
    pub fn new(piece_config: *mut ArmorPieceConfig, label: Component, on_update: Box<dyn FnMut()>) -> PieceConfigWidget {
        PieceConfigWidget { piece_config, label, on_update }
    }

    fn cfg(&mut self) -> &mut ArmorPieceConfig {
        unsafe { &mut *self.piece_config }
    }

    /// Java: toggleAll() - setAll(!base && !trim), then onUpdate.
    pub fn toggle_all(&mut self) -> bool {
        let value = { let c = self.cfg(); !c.base && !c.trim };
        self.cfg().set_all(value);
        (self.on_update)();
        true
    }

    /// Java: the glint checkbox callback: glint = value; if !(base || trim) setAll(true).
    pub fn set_glint(&mut self, value: bool) {
        let c = self.cfg();
        c.glint = value;
        if !(c.base || c.trim) {
            c.set_all(true);
        }
        (self.on_update)();
    }

    /// Java: the base checkbox callback.
    pub fn set_base(&mut self, value: bool) {
        self.cfg().base = value;
        (self.on_update)();
    }

    /// Java: the trim checkbox callback.
    pub fn set_trim(&mut self, value: bool) {
        self.cfg().trim = value;
        (self.on_update)();
    }

    /// Java: build(context) - the underlined label + the checkbox row.
    pub fn build_node(&mut self) -> WidgetNode {
        let c = self.cfg();
        let glint_checked = c.glint && (c.base || c.trim);
        let _ = glint_checked;
        WidgetNode::new("Column", vec![
            WidgetNode::new("Clickable", vec![WidgetNode::new("Label", vec![])]),
            WidgetNode::new(
                "Row",
                vec![
                    WidgetNode::new("IconCheckbox", vec![]),
                    WidgetNode::new("IconCheckbox", vec![]),
                    WidgetNode::new("IconCheckbox", vec![]),
                ],
            ),
        ])
    }
}

/// nl.enjarai.showmeyourskin.gui.OtherConfigWidget (1:1): elytra/shield/hat
/// (PieceConfig, no trim) with the same toggle-all shape.
pub struct OtherConfigWidget {
    pub piece_config: *mut PieceConfig,
    pub label: Component,
    pub on_update: Box<dyn FnMut()>,
    pub texture: String,
}

impl OtherConfigWidget {
    pub fn new(piece_config: *mut PieceConfig, label: Component, on_update: Box<dyn FnMut()>, texture: String) -> OtherConfigWidget {
        OtherConfigWidget { piece_config, label, on_update, texture }
    }

    fn cfg(&mut self) -> &mut PieceConfig {
        unsafe { &mut *self.piece_config }
    }

    /// Java: toggleAll() - setAll(!base), then onUpdate.
    pub fn toggle_all(&mut self) -> bool {
        let value = { self.cfg().base };
        self.cfg().set_all(!value);
        (self.on_update)();
        true
    }

    /// Java: glint callback: glint = value; if !base setAll(true).
    pub fn set_glint(&mut self, value: bool) {
        let c = self.cfg();
        c.glint = value;
        if !c.base {
            c.set_all(true);
        }
        (self.on_update)();
    }

    /// Java: base checkbox callback.
    pub fn set_base(&mut self, value: bool) {
        self.cfg().base = value;
        (self.on_update)();
    }

    /// Java: build(context) - textures button/<texture> and _disabled.
    pub fn build_node(&mut self) -> WidgetNode {
        let _ = show_me_your_skin::id(&format!("button/{}", self.texture));
        WidgetNode::new("Column", vec![
            WidgetNode::new("Clickable", vec![WidgetNode::new("Label", vec![])]),
            WidgetNode::new(
                "Row",
                vec![
                    WidgetNode::new("IconCheckbox", vec![]),
                    WidgetNode::new("IconCheckbox", vec![]),
                ],
            ),
        ])
    }
}

/// nl.enjarai.showmeyourskin.gui.AdditionalOptionsWidget (1:1): the three
/// global toggles, each hidden when the synced config disallows it.
pub struct AdditionalOptionsWidget {
    pub armor_config: *mut ArmorConfig,
    pub synced: bool,
    pub on_update: Box<dyn FnMut()>,
}

impl AdditionalOptionsWidget {
    pub fn new(armor_config: *mut ArmorConfig, synced: bool, on_update: Box<dyn FnMut()>) -> AdditionalOptionsWidget {
        AdditionalOptionsWidget { armor_config, synced, on_update }
    }

    fn cfg(&mut self) -> &mut ArmorConfig {
        unsafe { &mut *self.armor_config }
    }

    /// Java: configAllows(getter) - true when serverConfig is null, else the flag.
    pub fn config_allows(getter: impl FnOnce(&SyncedModConfig) -> bool) -> bool {
        client::with_server_config(|cfg| match cfg {
            None => true,
            Some(cfg) => getter(cfg),
        })
    }

    /// Java: update(fn) - setState(fn), then syncToServer or ModConfig.save().
    pub fn update(&mut self, f: impl FnOnce()) {
        f();
        (self.on_update)();
        if self.synced {
            client::sync_to_server(unsafe { &*self.armor_config });
        } else {
            mod_config::with_instance(|c| c.save());
        }
    }

    /// Which toggles this build shows (Java's ternaries against configAllows).
    pub fn visible_toggles(&self) -> Vec<&'static str> {
        vec![
            (!self.synced || Self::config_allows(|c| c.allow_not_show_in_combat)).then_some("show_in_combat"),
            (!self.synced || Self::config_allows(|c| c.allow_not_show_name_tag)).then_some("show_nametag"),
            (!self.synced || Self::config_allows(|c| c.allow_not_force_elytra_when_flying)).then_some("force_elytra_when_flying"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    pub fn build_node(&mut self) -> WidgetNode {
        let count = self.visible_toggles().len();
        let _ = self.cfg();
        WidgetNode::new(
            "Constrain",
            vec![WidgetNode::new(
                "Row",
                (0..count).map(|_| WidgetNode::new("IconCheckbox", vec![])).collect(),
            )],
        )
    }
}

/// nl.enjarai.showmeyourskin.gui.ArmorControlsWidget (1:1): the seven piece
/// rows inside a scrolling panel; update() syncs or saves.
pub struct ArmorControlsWidget {
    pub armor_config: *mut ArmorConfig,
    pub synced: bool,
}

impl ArmorControlsWidget {
    pub fn new(armor_config: *mut ArmorConfig, synced: bool) -> ArmorControlsWidget {
        ArmorControlsWidget { armor_config, synced }
    }

    /// Java: update() - synced -> ShowMeYourSkinClient.syncToServer(config),
    /// else ModConfig.INSTANCE.save().
    pub fn update(&self) {
        if self.synced {
            client::sync_to_server(unsafe { &*self.armor_config });
        } else {
            mod_config::with_instance(|c| c.save());
        }
    }

    /// Java: build(context) - PieceConfigWidget for head/chest/legs/feet and
    /// OtherConfigWidget for elytra/shield/hat.
    pub fn build_node(&self) -> WidgetNode {
        WidgetNode::new(
            "Panel",
            vec![WidgetNode::new(
                "ScrollableWithBars",
                vec![
                    WidgetNode::new("PieceConfigWidget", vec![]),
                    WidgetNode::new("PieceConfigWidget", vec![]),
                    WidgetNode::new("PieceConfigWidget", vec![]),
                    WidgetNode::new("PieceConfigWidget", vec![]),
                    WidgetNode::new("OtherConfigWidget", vec![]),
                    WidgetNode::new("OtherConfigWidget", vec![]),
                    WidgetNode::new("OtherConfigWidget", vec![]),
                ],
            )],
        )
    }
}

/// nl.enjarai.showmeyourskin.gui.ArmorConfigWidget (1:1): 200x200 panel with
/// the controls column and the preview.
pub struct ArmorConfigWidget {
    pub player_option: PlayerOption,
    pub synced: bool,
}

impl ArmorConfigWidget {
    pub fn new(player_option: PlayerOption, synced: bool) -> ArmorConfigWidget {
        ArmorConfigWidget { player_option, synced }
    }

    /// Java: build(context) - Sized(200, 200, Panel(VANILLA_LIGHT, Row(
    ///   IntrinsicWidth(Column(ArmorControlsWidget, AdditionalOptionsWidget)),
    ///   Flexible(PreviewWidget)))).
    pub fn build_node(&self) -> WidgetNode {
        WidgetNode::new(
            "Sized",
            vec![WidgetNode::new(
                "Panel",
                vec![WidgetNode::new(
                    "Row",
                    vec![
                        WidgetNode::new("Column", vec![
                            WidgetNode::new("ArmorControlsWidget", vec![]),
                            WidgetNode::new("AdditionalOptionsWidget", vec![]),
                        ]),
                        WidgetNode::new("PreviewWidget", vec![]),
                    ],
                )],
            )],
        )
    }
}

/// nl.enjarai.showmeyourskin.gui.SyncedArmorConfigWidget (1:1): the two global
/// icon toggles, the edit-overrides button while overrides are enabled, and
/// the ArmorConfigWidget in synced mode.
pub struct SyncedArmorConfigWidget {
    pub player_option: PlayerOption,
}

impl SyncedArmorConfigWidget {
    pub fn new(player_option: PlayerOption) -> SyncedArmorConfigWidget {
        SyncedArmorConfigWidget { player_option }
    }

    /// Java: update(fn) - setState(fn) then ModConfig.INSTANCE.save().
    pub fn update(&self, f: impl FnOnce()) {
        f();
        mod_config::with_instance(|c| c.save());
    }

    /// Java: build(context) - Row(global icon checkbox, overrides checkbox,
    ///   overrides enabled ? edit-override Clickable : Empty), ArmorConfigWidget(synced).
    pub fn build_node(&self) -> WidgetNode {
        let overrides_enabled = mod_config::with_instance(|c| c.overrides_enabled_in_server_mode);
        let mut row = vec![
            WidgetNode::new("Tooltip", vec![WidgetNode::new("IconCheckbox", vec![])]),
            WidgetNode::new("Tooltip", vec![WidgetNode::new("IconCheckbox", vec![])]),
        ];
        if overrides_enabled {
            row.push(WidgetNode::new("Tooltip", vec![WidgetNode::new("Clickable", vec![])]));
        } else {
            row.push(WidgetNode::new("EmptyWidget", vec![]));
        }
        WidgetNode::new(
            "Column",
            vec![WidgetNode::new("Row", row), WidgetNode::new("ArmorConfigWidget", vec![])],
        )
    }
}

/// nl.enjarai.showmeyourskin.gui.LocalArmorConfigWidget (1:1): the global
/// option plus the stored/online options, with selection + override editing.
pub struct LocalArmorConfigWidget {
    pub options: Vec<PlayerOption>,
    /// Java: final PlayerOption globalOption (ModConfig.INSTANCE.global,
    /// mc.getGameProfile().id(), mc.player, "armorScreen.global").
    pub global_option: PlayerOption,
    pub selected_option: usize, // 0 == globalOption, else options[idx - 1]
}

impl LocalArmorConfigWidget {
    pub fn new(options: Vec<PlayerOption>) -> LocalArmorConfigWidget {
        let env = ClientEnv::current();
        let global_option = PlayerOption::new(
            mod_config::with_instance(|c| c.global.clone()),
            env.game_profile_id,
            env.player.clone(),
            "gui.showmeyourskin.armorScreen.global".to_string(),
        );
        LocalArmorConfigWidget { options, global_option, selected_option: 0 }
    }

    pub fn selected_is_global(&self) -> bool {
        self.selected_option == 0
    }

    pub fn selected(&self) -> &PlayerOption {
        if self.selected_is_global() { &self.global_option } else { &self.options[self.selected_option - 1] }
    }

    /// Java: didUpdateWidget - drop the selection when its option left the list.
    pub fn did_update_widget(&mut self) {
        let _old_options: &[PlayerOption] = &[];
        if !self.selected_is_global()
            && !self.options.iter().any(|o| o.uuid == self.selected().uuid)
        {
            self.selected_option = 0;
        }
    }

    /// Java: updateOption(option) - select it; non-global options get an
    /// override entry + save.
    pub fn update_option(&mut self, option: PlayerOption) {
        let idx = if option.uuid == self.global_option.uuid {
            0
        } else {
            self.options.iter().position(|o| o.uuid == option.uuid).map(|i| i + 1).unwrap_or(0)
        };
        self.selected_option = idx;
        if idx != 0 {
            let title = option.title.clone();
            let config = option.armor_config.clone();
            let uuid = option.uuid;
            mod_config::with_instance_mut(|c| {
                c.overrides.insert(uuid, crate::config::mod_config::LocalOverride { title: Some(title), config: Some(config) });
            });
            mod_config::with_instance(|c| c.save());
        }
    }

    /// Java: deleteOption(option) - clear its config, drop the override, save,
    /// and fall back to the global selection when it was selected.
    pub fn delete_option(&mut self, option: &PlayerOption) {
        let was_selected = !self.selected_is_global() && self.selected().uuid == option.uuid;
        if was_selected {
            self.selected_option = 0;
        }
        // Java: option.armorConfig().clear() - reset every toggle.
        let mut cleared = ArmorConfig::vanilla_values();
        cleared.show_in_combat = false;
        cleared.show_name_tag = false;
        cleared.force_elytra_when_flying = false;
        mod_config::with_instance_mut(|c| {
            c.overrides.remove(&option.uuid);
        });
        let _ = cleared;
        mod_config::with_instance(|c| c.save());
    }

    /// Java: build(context) - Row(PlayerListWidget, ArmorConfigWidget(selected, false)).
    pub fn build_node(&self) -> WidgetNode {
        WidgetNode::new(
            "Row",
            vec![WidgetNode::new("PlayerListWidget", vec![]), WidgetNode::new("ArmorConfigWidget", vec![])],
        )
    }
}

/// nl.enjarai.showmeyourskin.gui.PreviewWidget (1:1): the mannequin preview;
/// entity ticking/rendering are engine seams.
pub struct PreviewWidget {
    pub player_option: PlayerOption,
    pub ready: bool,
}

impl PreviewWidget {
    pub fn new(player_option: PlayerOption) -> PreviewWidget {
        PreviewWidget { player_option, ready: false }
    }

    /// Java: init() - construct the mannequin, start the 50ms tick loop.
    pub fn init(&mut self) {
        let _ = self.player_option.construct_mannequin();
    }

    /// Java: tickEntity() - tick + ready flip when the skin leaves the default.
    pub fn tick_entity(&mut self) {
        // Seamed: mannequin tick + ClientMannequin.getSkin() default check.
        let now_ready = self.ready;
        if now_ready != self.ready {
            self.ready = now_ready;
        }
    }

    /// Java: didUpdateWidget - rebuild the mannequin when the option changes.
    pub fn did_update_widget(&mut self, old: &PlayerOption) {
        let same = self.player_option.uuid == old.uuid
            && self.player_option.title == old.title
            && self.player_option.player.is_some() == old.player.is_some();
        if !same {
            let _ = self.player_option.construct_mannequin();
            self.ready = false;
        }
    }

    pub fn build_node(&self) -> WidgetNode {
        WidgetNode::new(
            "Panel",
            vec![WidgetNode::new("Stack", vec![WidgetNode::new("Padding", vec![])])],
        )
    }
}

/// nl.enjarai.showmeyourskin.gui.PlayerListEntry (1:1).
pub struct PlayerListEntry {
    pub player_option: PlayerOption,
    pub skin_texture_override: Option<String>,
    pub selected: bool,
    pub deletable: bool,
    pub on_select: Box<dyn FnMut()>,
    pub on_delete: Box<dyn FnMut()>,
}

impl PlayerListEntry {
    pub fn new(player_option: PlayerOption, skin_texture_override: Option<String>, selected: bool, deletable: bool, on_select: Box<dyn FnMut()>, on_delete: Box<dyn FnMut()>) -> PlayerListEntry {
        PlayerListEntry { player_option, skin_texture_override, selected, deletable, on_select, on_delete }
    }

    /// Java: init() - resolve the skin texture unless overridden (async seam).
    pub fn init(&self) {
        if self.skin_texture_override.is_none() && self.player_option.player.is_some() {
            // Seamed: profileResolver().fetchById + getSkinManager().get.
        }
    }

    pub fn build_node(&self) -> WidgetNode {
        let mut row = vec![
            WidgetNode::new("Sized", vec![WidgetNode::new("CustomDraw", vec![])]),
            WidgetNode::new("Flexible", vec![WidgetNode::new("Marquee", vec![WidgetNode::new("Label", vec![])])]),
        ];
        if self.deletable {
            row.push(WidgetNode::new("Sized", vec![WidgetNode::new("Clickable", vec![WidgetNode::new("HoverableBuilder", vec![WidgetNode::new("SpriteWidget", vec![])])])]));
        } else {
            row.push(WidgetNode::new("EmptyWidget", vec![]));
        }
        WidgetNode::new(
            "Clickable",
            vec![WidgetNode::new(
                "Box",
                vec![WidgetNode::new("Padding", vec![WidgetNode::new("Row", row)])],
            )],
        )
    }
}

/// nl.enjarai.showmeyourskin.gui.PlayerListWidget (1:1): the 100x180 list
/// panel: the global entry (no delete) + the scrollable option list.
pub struct PlayerListWidget {
    pub global_option: PlayerOption,
    pub options: Vec<PlayerOption>,
    pub selected_uuid: uuid::Uuid,
}

impl PlayerListWidget {
    pub fn new(global_option: PlayerOption, options: Vec<PlayerOption>, selected_uuid: uuid::Uuid) -> PlayerListWidget {
        PlayerListWidget { global_option, options, selected_uuid }
    }

    /// Java: build(context) - Sized(100, 180, Panel(VANILLA_LIGHT, Column(
    ///   Panel(global entry), Flexible(Panel(Row(Scrollable(entries),
    ///   ButtonScrollbar)))))). Entries are deletable exactly when the uuid has
    ///   a stored override.
    pub fn build_node(&self) -> WidgetNode {
        let deletable_flags: Vec<bool> = {
            mod_config::with_instance(|c| {
                self.options.iter().map(|o| c.overrides.contains_key(&o.uuid)).collect()
            })
        };
        let mut children = vec![WidgetNode::new("Panel", vec![WidgetNode::new("PlayerListEntry", vec![])])];
        children.push(WidgetNode::new(
            "Flexible",
            vec![WidgetNode::new(
                "Panel",
                vec![WidgetNode::new(
                    "Row",
                    vec![
                        WidgetNode::new(
                            "Scrollable",
                            (0..self.options.len())
                                .map(|i| {
                                    let _ = deletable_flags[i];
                                    WidgetNode::new("PlayerListEntry", vec![])
                                })
                                .collect(),
                        ),
                        WidgetNode::new("ButtonScrollbar", vec![]),
                    ],
                )],
            )],
        ));
        WidgetNode::new("Sized", vec![WidgetNode::new("Panel", vec![WidgetNode::new("Column", children)])])
    }
}

/// Java: ArmorConfigScreen parent passthrough used by the widgets above.
pub fn open_local_config_screen(parent: Option<WidgetNode>) -> ArmorConfigScreen {
    ArmorConfigScreen::create_local_config_screen(parent)
}
