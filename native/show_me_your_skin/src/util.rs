//! nl.enjarai.showmeyourskin.util (1:1): CombatLogger, ArmorContext,
//! MixinContext, ArmorConfigComponent. Entity/component types are engine
//! seams; the logic shapes are exact.

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread::{ThreadId, current};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::armor_config::ArmorConfig;
use crate::config::mod_config;
use crate::config::mod_config_server;
use crate::seams::{self, EntityRef};
use crate::show_me_your_skin;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// nl.enjarai.showmeyourskin.util.CombatLogger (1:1). Java holds a singleton
/// with a HashMap<UUID, Long>; Rust uses a global instance over a thread-safe
/// map (the Java access sites are the client thread).
pub struct CombatLogger {
    in_combat: Mutex<HashMap<uuid::Uuid, u64>>,
}

impl CombatLogger {
    /// Java: public static final CombatLogger INSTANCE = new CombatLogger();
    pub fn new() -> CombatLogger {
        CombatLogger {
            in_combat: Mutex::new(HashMap::new()),
        }
    }

    /// Java: isInCombat(uuid) - true while the last trigger is inside
    /// combatCooldown seconds.
    pub fn is_in_combat(&self, uuid: uuid::Uuid) -> bool {
        let last_trigger = self.in_combat.lock().unwrap().get(&uuid).copied();
        if let Some(last_trigger) = last_trigger {
            let cooldown_ms =
                (mod_config::with_instance(|c| c.combat_cooldown) as f64 * 1000.0) as u64;
            if last_trigger > now_millis().saturating_sub(cooldown_ms) {
                return true;
            }
            self.in_combat.lock().unwrap().remove(&uuid);
        }
        false
    }

    /// Java: getFade(uuid) - the current fade factor, clamped 0..1.
    pub fn get_fade(&self, uuid: uuid::Uuid) -> f32 {
        let last_trigger = self.in_combat.lock().unwrap().get(&uuid).copied();
        if let Some(last_trigger) = last_trigger {
            let (cooldown, fade_out) =
                mod_config::with_instance(|c| (c.combat_cooldown, c.fade_out_time));
            let fade_start = last_trigger as i64 + (cooldown as f64 * 1000.0) as i64
                - (fade_out as f64 * 1000.0) as i64;
            let time_since_combat = (now_millis() as i64 - fade_start) as f32;

            // Calculate the current transparency using the time since combat ended
            // and the fade out duration, then clamp between 0 and 1 to ensure the
            // final transparency is calculated properly.
            return seams::mth_clamp(1.0 - (time_since_combat / 1000.0) / fade_out, 0.0, 1.0);
        }
        1.0
    }

    /// Java: triggerCombat(uuid).
    pub fn trigger_combat(&self, uuid: uuid::Uuid) {
        self.in_combat.lock().unwrap().insert(uuid, now_millis());
    }
}

/// Java: public static final CombatLogger INSTANCE.
pub static COMBAT_LOGGER: std::sync::LazyLock<CombatLogger> =
    std::sync::LazyLock::new(CombatLogger::new);

/// nl.enjarai.showmeyourskin.util.ArmorContext (1:1): the mixin context for
/// armor-relevant render sites.
#[derive(Clone)]
pub struct ArmorContext {
    slot: crate::config::hideable_equipment::HideableEquipment,
    entity: EntityRef,
}

impl ArmorContext {
    pub fn new(
        slot: crate::config::hideable_equipment::HideableEquipment,
        entity: EntityRef,
    ) -> ArmorContext {
        ArmorContext { slot, entity }
    }

    pub fn get_slot(&self) -> &crate::config::hideable_equipment::HideableEquipment {
        &self.slot
    }

    pub fn get_entity(&self) -> &EntityRef {
        &self.entity
    }

    /// Java: shouldModify() - getEntity() instanceof Player.
    pub fn should_modify(&self) -> bool {
        self.entity.is_player
    }
}

/// nl.enjarai.showmeyourskin.util.MixinContext<T> (1:1): a thread-local current
/// context with the set/get/clear/getAndClear shape. ARMOR and ENTITY are the
/// two static instances.
pub struct MixinContext<T: Send> {
    // Java ThreadLocal<T>. Rust std has no generic thread-local statics, so the
    // seam keys slots by thread id: per-thread isolation, same API shape.
    current_context: Mutex<HashMap<ThreadId, Option<T>>>,
}

impl<T: Send + Clone> MixinContext<T> {
    pub fn new() -> MixinContext<T> {
        MixinContext {
            current_context: Mutex::new(HashMap::new()),
        }
    }

    /// Java: setContext(context).
    pub fn set_context(&self, context: T) {
        self.current_context
            .lock()
            .unwrap()
            .insert(current().id(), Some(context));
    }

    /// Java: getContext() (@Nullable).
    pub fn get_context(&self) -> Option<T> {
        self.current_context
            .lock()
            .unwrap()
            .get(&current().id())
            .and_then(|c| c.as_ref().cloned())
    }

    /// Java: clearContext().
    pub fn clear_context(&self) {
        if let Some(slot) = self
            .current_context
            .lock()
            .unwrap()
            .get_mut(&current().id())
        {
            *slot = None;
        }
    }

    /// Java: getAndClearContext() (@Nullable).
    pub fn get_and_clear_context(&self) -> Option<T> {
        self.current_context
            .lock()
            .unwrap()
            .get_mut(&current().id())
            .and_then(|c| c.take())
    }
}

/// Java: public static final MixinContext<ArmorContext> ARMOR = new MixinContext<>();
pub static CONTEXT_ARMOR: std::sync::LazyLock<MixinContext<ArmorContext>> =
    std::sync::LazyLock::new(MixinContext::new);
/// Java: public static final MixinContext<LivingEntity> ENTITY = new MixinContext<>();
pub static CONTEXT_ENTITY: std::sync::LazyLock<MixinContext<EntityRef>> =
    std::sync::LazyLock::new(MixinContext::new);

/// nl.enjarai.showmeyourskin.util.ArmorConfigComponent (1:1). CCA is an engine
/// seam: readData/writeData take the NBT ValueInput/ValueOutput form, and the
/// sync hooks ride the net stage.
pub struct ArmorConfigComponent {
    config: ArmorConfig,
}

impl ArmorConfigComponent {
    /// Java: KEYED_CONFIG = ArmorConfig.ENDEC.keyed("config", ArmorConfig.VANILLA_VALUES).
    pub const KEYED_CONFIG: &'static str = "config";

    pub fn new(config: ArmorConfig) -> ArmorConfigComponent {
        ArmorConfigComponent { config }
    }

    /// Java: readData(readView) -> setConfig(readView.get(KEYED_CONFIG)).
    pub fn read_data(&mut self, value: Option<&ArmorConfig>) {
        self.set_config(value.cloned().unwrap_or_else(ArmorConfig::vanilla_values));
    }

    /// Java: writeData(writeView) -> writeView.put(KEYED_CONFIG, config).
    pub fn write_data(&self) -> ArmorConfig {
        self.get_config().clone()
    }

    pub fn get_config(&self) -> &ArmorConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: ArmorConfig) {
        self.config = config;
    }

    /// Java: ensureValid() - clamp to the server's allowed toggles.
    pub fn ensure_valid(&mut self) {
        mod_config_server::with_instance(|i| {
            if let Some(c) = i {
                if !c.synced.allow_not_show_in_combat {
                    self.config.show_in_combat = true;
                }
                if !c.synced.allow_not_show_name_tag {
                    self.config.show_name_tag = true;
                }
                if !c.synced.allow_not_force_elytra_when_flying {
                    self.config.force_elytra_when_flying = true;
                }
            }
        });
    }

    /// Java: shouldSyncWith(player) -> ShowMeYourSkin.NET_CHANNEL.canSendToPlayer(player).
    pub fn should_sync_with(&self, _player: &EntityRef) -> bool {
        show_me_your_skin::net_channel().can_send_to_player()
    }

    /// Java: isRequiredOnClient() -> false.
    pub fn is_required_on_client(&self) -> bool {
        false
    }
}

/// nl.enjarai.showmeyourskin.Components (1:1). Java: ComponentKey
/// ARMOR_CONFIG = ComponentRegistry.getOrCreate(ShowMeYourSkin.id("armor_config"), ...)
/// registered for players with RespawnCopyStrategy.ALWAYS_COPY.
pub struct Components;

impl Components {
    /// The ComponentKey's id, resolved once (Java static init).
    pub fn armor_config_key() -> seams::ComponentRegistry {
        seams::ComponentRegistry {
            key_id: show_me_your_skin::id("armor_config"),
        }
    }

    /// Java: registerEntityComponentFactories(registry) - players start at
    /// VANILLA_VALUES and the component always copies on respawn.
    pub fn register_entity_component_factories() -> ArmorConfigComponent {
        ArmorConfigComponent::new(ArmorConfig::vanilla_values())
    }

    /// Java: RespawnCopyStrategy.ALWAYS_COPY, kept for parity.
    pub const RESPAWN_COPY_STRATEGY: &'static str = "ALWAYS_COPY";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_logger_window_and_fade() {
        let logger = CombatLogger::new();
        let p = uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        assert!(!logger.is_in_combat(p));
        assert_eq!(logger.get_fade(p), 1.0);

        mod_config::with_instance_mut(|c| {
            c.combat_cooldown = 10.0;
            c.fade_out_time = 2.0;
        });
        logger.trigger_combat(p);
        assert!(logger.is_in_combat(p));
        // just after trigger, the fade window has not opened yet (fade starts at
        // cooldown - fadeOutTime = 8s), so timeSinceCombat is negative and the
        // clamp holds the value at 1.
        assert_eq!(logger.get_fade(p), 1.0);

        // simulate an old trigger: fade fully done. Java's isInCombat drops
        // expired entries, so read the fade on a fresh uuid whose backdated
        // trigger is still in the map.
        logger.trigger_combat(p);
        logger
            .in_combat
            .lock()
            .unwrap()
            .insert(p, now_millis().saturating_sub(11_000));
        assert!(!logger.is_in_combat(p));
        let q = uuid::Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap();
        logger.trigger_combat(q);
        logger
            .in_combat
            .lock()
            .unwrap()
            .insert(q, now_millis().saturating_sub(11_000));
        assert_eq!(logger.get_fade(q), 0.0);
    }

    #[test]
    fn armor_context_modifies_only_players() {
        let u = uuid::Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();
        let ctx = ArmorContext::new(
            crate::config::hideable_equipment::HideableEquipment::Shield,
            EntityRef {
                uuid: u,
                is_player: true,
            },
        );
        assert!(ctx.should_modify());
        let mob = ArmorContext::new(
            crate::config::hideable_equipment::HideableEquipment::Head,
            EntityRef {
                uuid: u,
                is_player: false,
            },
        );
        assert!(!mob.should_modify());
    }

    #[test]
    fn mixin_context_set_get_clear() {
        CONTEXT_ARMOR.set_context(ArmorContext::new(
            crate::config::hideable_equipment::HideableEquipment::Feet,
            EntityRef {
                uuid: uuid::Uuid::nil(),
                is_player: true,
            },
        ));
        assert!(CONTEXT_ARMOR.get_context().is_some());
        assert!(CONTEXT_ARMOR.get_and_clear_context().is_some());
        assert!(CONTEXT_ARMOR.get_context().is_none());

        let e = EntityRef {
            uuid: uuid::Uuid::nil(),
            is_player: false,
        };
        CONTEXT_ENTITY.set_context(e.clone());
        assert_eq!(CONTEXT_ENTITY.get_context(), Some(e));
        CONTEXT_ENTITY.clear_context();
        assert_eq!(CONTEXT_ENTITY.get_context(), None);
    }

    #[test]
    fn component_ensure_valid_clamps_to_server() {
        mod_config_server::set_instance(mod_config_server::ModConfigServer::default());
        let mut c = ArmorConfigComponent::new(ArmorConfig::vanilla_values());
        c.config.show_in_combat = false;
        c.config.show_name_tag = false;
        c.config.force_elytra_when_flying = false;
        // ModConfigServer default is synced(false,false,false): everything forced true
        c.ensure_valid();
        assert!(c.config.show_in_combat);
        assert!(c.config.show_name_tag);
        assert!(c.config.force_elytra_when_flying);
        assert!(!c.is_required_on_client());

        // readData default path: missing value -> VANILLA_VALUES
        let mut d = ArmorConfigComponent::new(ArmorConfig::default());
        d.read_data(None);
        assert_eq!(d.write_data(), ArmorConfig::vanilla_values());
    }

    #[test]
    fn components_registry_key() {
        let key = Components::armor_config_key();
        assert_eq!(key.key_id, "showmeyourskin/armor_config");
        assert_eq!(Components::RESPAWN_COPY_STRATEGY, "ALWAYS_COPY");
        let factory = Components::register_entity_component_factories();
        assert_eq!(factory.get_config(), &ArmorConfig::vanilla_values());
    }
}
