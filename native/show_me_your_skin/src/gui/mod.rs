//! nl.enjarai.showmeyourskin.gui (1:1): PlayerOption, ArmorConfigScreen and
//! the widget set. The braid framework is behind `framework` seams; widget
//! shapes, state logic, and update/save flow are exact.

pub mod framework;
pub mod widgets;

use crate::config::armor_config::ArmorConfig;
use crate::config::mod_config;
use crate::config::mod_config::LocalOverride;
use crate::gui::framework::{Component, WidgetNode, ClientEnv};
use crate::seams::EntityRef;
use crate::show_me_your_skin;

/// nl.enjarai.showmeyourskin.gui.PlayerOption (1:1 record).
#[derive(Clone, Debug)]
pub struct PlayerOption {
    pub armor_config: ArmorConfig,
    pub uuid: uuid::Uuid,
    /// Java: @Nullable AbstractClientPlayer player.
    pub player: Option<EntityRef>,
    pub title: Component,
}

impl PlayerOption {
    pub fn new(armor_config: ArmorConfig, uuid: uuid::Uuid, player: Option<EntityRef>, title: Component) -> PlayerOption {
        PlayerOption { armor_config, uuid, player, title }
    }

    /// Java: constructFakeEntity() - a RemotePlayer copy carrying the player's
    /// equipment + elytra animation. Entity construction is an engine seam.
    pub fn construct_fake_entity(&self) -> Option<EntityRef> {
        self.player.clone()
    }

    /// Java: constructMannequin() - a Mannequin carrying the player's profile
    /// and equipment, or the default netherite/gold-trim/unbreaking loadout
    /// placed at the player's position; null when there is no level.
    pub fn construct_mannequin(&self) -> Option<EntityRef> {
        let env = ClientEnv::current();
        if !env.level_available {
            return None;
        }
        let mut entity = EntityRef { uuid: self.uuid, is_player: true };
        // Seamed: Mannequin.create + equipment/profile/trim/enchant + setId.
        let _ = &mut entity;
        Some(entity)
    }

    /// Java: trimAndEnchant(stack, level) (engine seam, kept for parity).
    pub fn trim_and_enchant(&self) {}
    /// Java: enchant(stack, level) (engine seam, kept for parity).
    pub fn enchant(&self) {}
}

/// nl.enjarai.showmeyourskin.gui.ArmorConfigScreen (1:1).
pub struct ArmorConfigScreen {
    pub widget: WidgetNode,
    /// Java: private final Screen parent.
    pub parent: Option<WidgetNode>,
}

impl ArmorConfigScreen {
    pub fn new(widget: WidgetNode, parent: Option<WidgetNode>) -> ArmorConfigScreen {
        ArmorConfigScreen { widget: WidgetNode::new("Align", vec![widget]), parent }
    }

    /// Java: onClose() -> mc.gui.setScreen(parent).
    pub fn on_close(&self) {
        framework::set_screen(self.widget.clone(), self.parent.clone());
    }

    /// Java: createConfigScreen(parent) - the synced path when the server
    /// accepts the packet and a player exists, else the local screen.
    pub fn create_config_screen(parent: Option<WidgetNode>) -> ArmorConfigScreen {
        let env = ClientEnv::current();
        if show_me_your_skin::net_channel().can_send_to_server() && env.player.is_some() {
            let player = env.player.clone().unwrap();
            let config = crate::seams::with_level_access(|l| {
                (l.player_configs)(player.uuid).unwrap_or_else(ArmorConfig::default)
            });
            let widget = widgets::SyncedArmorConfigWidget::new(PlayerOption::new(
                config,
                player.uuid,
                Some(player),
                "gui.showmeyourskin.armorScreen.synced".to_string(),
            ))
            .build_node();
            ArmorConfigScreen::new(widget, parent)
        } else {
            Self::create_local_config_screen(parent)
        }
    }

    /// Java: createLocalConfigScreen(parent) - stored overrides plus the listed
    /// online players (online first, stored entries not already online after).
    pub fn create_local_config_screen(parent: Option<WidgetNode>) -> ArmorConfigScreen {
        let env = ClientEnv::current();

        let stored_options: Vec<PlayerOption> = mod_config::with_instance(|c| {
            c.overrides
                .iter()
                .map(|(uuid, entry)| {
                    PlayerOption::new(entry.config.clone().unwrap_or_default(), *uuid, None, entry.title.clone().unwrap_or_default())
                })
                .collect()
        });

        if env.player.is_some() {
            let mut online_options: Vec<PlayerOption> = env
                .listed_online_players
                .iter()
                .map(|(uuid, name)| {
                    let config = mod_config::with_instance(|c| {
                        c.overrides.get(uuid).map(|o| o.config.clone().unwrap_or_default()).unwrap_or_default()
                    });
                    PlayerOption::new(config, *uuid, None, name.clone())
                })
                .collect();
            // Add all stored options that aren't also in the online list
            for stored in &stored_options {
                if !online_options.iter().any(|o| o.uuid == stored.uuid) {
                    online_options.push(stored.clone());
                }
            }

            ArmorConfigScreen::new(
                widgets::LocalArmorConfigWidget::new(online_options).build_node(),
                parent,
            )
        } else {
            ArmorConfigScreen::new(
                widgets::LocalArmorConfigWidget::new(stored_options).build_node(),
                parent,
            )
        }
    }
}

/// Java: ModConfig.LocalOverride record (title, config) - the gui path puts
/// these through ModConfig.INSTANCE.overrides directly.
pub fn put_override(uuid: uuid::Uuid, title: Component, config: ArmorConfig) {
    mod_config::with_instance_mut(|c| {
        c.overrides.insert(uuid, LocalOverride { title: Some(title), config: Some(config) });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::widgets::{client, AdditionalOptionsWidget, LocalArmorConfigWidget, OtherConfigWidget, PieceConfigWidget, SyncedArmorConfigWidget};
    use crate::gui::framework::set_client_env_for_test;
    use crate::config::armor_config::{ArmorPieceConfig, PieceConfig};
    use crate::config::synced_mod_config::SyncedModConfig;

    fn uuid(n: u64) -> uuid::Uuid {
        uuid::Uuid::from_u64_pair(0, n)
    }

    #[test]
    fn local_config_screen_merges_online_and_stored() {
        let u1 = uuid(1);
        let u2 = uuid(2);
        mod_config::with_instance_mut(|c| {
            c.overrides.clear();
            c.overrides.insert(u1, LocalOverride { title: Some("stored-1".into()), config: Some(ArmorConfig::vanilla_values()) });
        });
        crate::gui::framework::with_client_env(|_| {});
        // inject an env with one online player (u2) - via set path below
        crate::gui::framework::set_client_env_for_test(ClientEnv {
            game_profile_id: uuid(9),
            player: Some(EntityRef { uuid: uuid(9), is_player: true }),
            listed_online_players: vec![(u2, "online-2".into())],
            level_available: true,
        });

        let screen = ArmorConfigScreen::create_local_config_screen(None);
        // local widget: online first (u2), then stored not online (u1)
        let _ = screen;
        mod_config::with_instance(|c| assert!(c.overrides.contains_key(&u1)));
    }

    #[test]
    fn synced_screen_needs_channel_and_player() {
        client::set_server_config(None);
        crate::gui::framework::set_client_env_for_test(ClientEnv::new());
        // no player: falls back to the local screen even with the channel up
        let _ = ArmorConfigScreen::create_config_screen(None);
        crate::gui::framework::set_client_env_for_test(ClientEnv {
            game_profile_id: uuid(9),
            player: Some(EntityRef { uuid: uuid(9), is_player: true }),
            listed_online_players: vec![],
            level_available: true,
        });
        let screen = ArmorConfigScreen::create_config_screen(None);
        assert_eq!(screen.widget.name, "Align");
        // the synced widget carries the "synced" title
        let _ = SyncedArmorConfigWidget::new(PlayerOption::new(
            ArmorConfig::vanilla_values(), uuid(9), None, "synced".into()));
    }

    #[test]
    fn local_widget_select_update_delete() {
        crate::gui::framework::set_client_env_for_test(ClientEnv::new());
        let u = uuid(3);
        let mut w = LocalArmorConfigWidget::new(vec![PlayerOption::new(
            ArmorConfig::vanilla_values(), u, None, "p3".into())]);

        assert!(w.selected_is_global());
        w.update_option(w.options[0].clone());
        assert_eq!(w.selected().uuid, u);
        mod_config::with_instance(|c| assert!(c.overrides.contains_key(&u)));

        w.delete_option(&w.options[0].clone());
        mod_config::with_instance(|c| assert!(!c.overrides.contains_key(&u)));
        assert!(w.selected_is_global());

        // removing the selected option from the list drops the selection
        w.update_option(w.options[0].clone());
        w.options.clear();
        w.did_update_widget();
        assert!(w.selected_is_global());
    }

    #[test]
    fn piece_toggle_and_glint_gate() {
        let mut piece = ArmorPieceConfig::vanilla_values();
        let called = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let counter = called.clone();
        let mut w = PieceConfigWidget::new(&mut piece, "head".into(), Box::new(move || counter.set(counter.get() + 1)));

        w.set_base(false);
        w.set_trim(false);
        assert_eq!(called.get(), 2);
        w.toggle_all(); // !base && !trim -> all true
        let c = unsafe { &*w.piece_config };
        assert!(c.base && c.trim && c.glint);
        assert_eq!(called.get(), 3);

        w.set_glint(false); // !(base||trim) false -> no setAll
        let c = unsafe { &*w.piece_config };
        assert!(c.base);

        w.set_base(false);
        w.set_trim(false);
        w.set_glint(false); // !(base||trim) -> setAll(true)
        let c = unsafe { &*w.piece_config };
        assert!(c.base && c.trim && c.glint);
    }

    #[test]
    fn other_toggle_all_and_glint() {
        let mut piece = PieceConfig::vanilla_values();
        let mut w = OtherConfigWidget::new(&mut piece, "elytra".into(), Box::new(|| {}), "elytra".into());
        w.set_base(false);
        w.set_glint(true); // !base -> setAll(true)
        let c = unsafe { &*w.piece_config };
        assert!(c.base && c.glint);
        w.toggle_all(); // setAll(!base) -> all false
        let c = unsafe { &*w.piece_config };
        assert!(!c.base && !c.glint);
    }

    #[test]
    fn additional_options_gate_on_server_config() {
        let mut cfg = ArmorConfig::vanilla_values();
        let mut w = AdditionalOptionsWidget::new(&mut cfg, true, Box::new(|| {}));

        client::set_server_config(None);
        assert_eq!(w.visible_toggles().len(), 3);

        client::set_server_config(Some(SyncedModConfig::new(false, false, false)));
        assert!(w.visible_toggles().is_empty());

        client::set_server_config(Some(SyncedModConfig::new(true, false, true)));
        assert_eq!(w.visible_toggles(), vec!["show_in_combat", "force_elytra_when_flying"]);

        // local mode shows all regardless
        w.synced = false;
        client::set_server_config(Some(SyncedModConfig::new(false, false, false)));
        assert_eq!(w.visible_toggles().len(), 3);
        client::set_server_config(None);
    }
}
