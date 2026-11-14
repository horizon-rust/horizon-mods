//! Upstream `mixin/HudMixin.java`, 1:1.
/*
 * Copyright (C) Lortseam and contributors.
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see <https://www.gnu.org/licenses/>.
 */

use std::cell::RefCell;
use std::rc::Rc;

/// `net.minecraft.world.entity.PlayerRideableJumping` (token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerRideableJumping(pub u64);

/// `net.minecraft.world.entity.LivingEntity` (token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LivingEntity(pub u64);

/// The shadowed `Hud` state and the player/game-mode surface the handlers
/// read (`minecraft`, `getPlayerVehicleWithHealth`, `getVehicleMaxHearts`,
/// `gameMode`, `options.keyJump`, `minecraft.player`).
pub trait HudMixinEnv {
    /// `minecraft.gameMode.canHurtPlayer()`.
    fn can_hurt_player(&self) -> bool;
    /// `minecraft.gameMode.hasExperience()`.
    fn game_mode_has_experience(&self) -> bool;
    /// `minecraft.options.keyJump.isDown()`.
    fn key_jump_is_down(&self) -> bool;
    /// `minecraft.player.jumpableVehicle()`.
    fn player_jumpable_vehicle(&self) -> Option<PlayerRideableJumping>;
    /// `minecraft.player.getJumpRidingScale()`.
    fn player_jump_riding_scale(&self) -> f32;
    /// Shadowed `getPlayerVehicleWithHealth()`.
    fn get_player_vehicle_with_health(&self) -> Option<LivingEntity>;
    /// Shadowed `getVehicleMaxHearts(LivingEntity)`.
    fn get_vehicle_max_hearts(&self, entity: LivingEntity) -> i32;
}

/// `@Mixin(Hud.class) public abstract class HudMixin`.
#[derive(Clone)]
pub struct HudMixin {
    env: Rc<RefCell<dyn HudMixinEnv>>,
}

impl HudMixin {
    pub fn new(env: Rc<RefCell<dyn HudMixinEnv>>) -> Self {
        Self { env }
    }

    /// `@ModifyVariable(method = "extractVehicleHealth", at = @At(value =
    /// "STORE"), name = "yLine1") private int
    /// bettermounthud$moveMountHealthUp(int y)`.
    ///
    /// The mount health row moves up by the vanilla food-bar gap whenever
    /// the game mode would hurt the player, so both rows can render together.
    pub fn bettermounthud_move_mount_health_up(&self, mut y: i32) -> i32 {
        if self.env.borrow().can_hurt_player() {
            y -= 10;
        }
        y
    }

    /// `@Redirect(method = "extractPlayerHealth", at = @At(value = "INVOKE",
    /// target = "Lnet/minecraft/client/gui/Hud;getVehicleMaxHearts(Lnet/
    /// minecraft/world/entity/LivingEntity;)I")) private int
    /// bettermounthud$alwaysRenderFood(Hud hud, LivingEntity entity)`.
    ///
    /// Redirects the vehicle-max-hearts call to 0 so the player food bar
    /// always renders while riding.
    pub fn bettermounthud_always_render_food(
        &self,
        _hud: &(),
        _entity: LivingEntity,
    ) -> i32 {
        0
    }

    /// `@ModifyVariable(method = "getAirBubbleYLine", at = @At(value =
    /// "HEAD"), ordinal = 0, argsOnly = true) private int
    /// bettermounthud$moveAirUp(int heartCount)`.
    ///
    /// While riding a mount with health, the air bubble row sits under the
    /// vehicle's hearts instead of the player's.
    pub fn bettermounthud_move_air_up(&self, heart_count: i32) -> i32 {
        let entity = self.env.borrow().get_player_vehicle_with_health();
        if let Some(entity) = entity {
            return self.env.borrow().get_vehicle_max_hearts(entity);
        }
        heart_count
    }

    /// `@Redirect(method = "nextContextualInfoState", at = @At(value =
    /// "INVOKE", target = "Lnet/minecraft/client/player/LocalPlayer;
    /// jumpableVehicle()Lnet/minecraft/world/entity/PlayerRideableJumping;"))
    /// private PlayerRideableJumping bettermounthud$switchBar(LocalPlayer
    /// player)`.
    ///
    /// The jump bar yields to the experience bar unless experience is hidden,
    /// the jump key is held, or the mount's jump meter is already charging.
    pub fn bettermounthud_switch_bar(
        &self,
        player: PlayerRideableJumping,
    ) -> Option<PlayerRideableJumping> {
        let jumpable_vehicle = player;
        let env = self.env.borrow();
        if !env.game_mode_has_experience()
            || env.key_jump_is_down()
            || env.player_jump_riding_scale() > 0.0
        {
            return Some(jumpable_vehicle);
        }
        None
    }

    /// `@Redirect(method = "extractHotbarAndDecorations", at = @At(value =
    /// "INVOKE", target = "Lnet/minecraft/client/multiplayer/
    /// MultiPlayerGameMode;hasExperience()Z")) private boolean
    /// bettermounthud$renderExperienceLevel(MultiPlayerGameMode gameMode)`.
    ///
    /// The experience level renders while riding only when the jump bar
    /// is not taking its place.
    pub fn bettermounthud_render_experience_level(&self) -> bool {
        let env = self.env.borrow();
        let has_experience = env.game_mode_has_experience();
        let jumpable_vehicle = env.player_jumpable_vehicle();
        let key_jump_down = env.key_jump_is_down();
        let jump_riding_scale = env.player_jump_riding_scale();
        has_experience
            && ((jumpable_vehicle.is_some() && !key_jump_down && jump_riding_scale <= 0.0)
                || jumpable_vehicle.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    #[derive(Clone)]
    struct TestEnv {
        can_hurt: bool,
        has_experience: bool,
        key_jump_down: bool,
        jumpable: Option<PlayerRideableJumping>,
        jump_scale: f32,
        vehicle: Option<LivingEntity>,
        vehicle_hearts: i32,
    }

    impl HudMixinEnv for TestEnv {
        fn can_hurt_player(&self) -> bool {
            self.can_hurt
        }
        fn game_mode_has_experience(&self) -> bool {
            self.has_experience
        }
        fn key_jump_is_down(&self) -> bool {
            self.key_jump_down
        }
        fn player_jumpable_vehicle(&self) -> Option<PlayerRideableJumping> {
            self.jumpable
        }
        fn player_jump_riding_scale(&self) -> f32 {
            self.jump_scale
        }
        fn get_player_vehicle_with_health(&self) -> Option<LivingEntity> {
            self.vehicle
        }
        fn get_vehicle_max_hearts(&self, _entity: LivingEntity) -> i32 {
            self.vehicle_hearts
        }
    }

    fn mixin(env: &TestEnv) -> HudMixin {
        HudMixin::new(Rc::new(RefCell::new(env.clone())))
    }

    #[test]
    fn mount_health_moves_up_only_when_the_player_can_be_hurt() {
        let env = TestEnv::default();
        assert_eq!(mixin(&env).bettermounthud_move_mount_health_up(30), 30);
        let env = TestEnv {
            can_hurt: true,
            ..Default::default()
        };
        assert_eq!(mixin(&env).bettermounthud_move_mount_health_up(30), 20);
    }

    #[test]
    fn the_food_redirect_always_reports_zero_vehicle_hearts() {
        let env = TestEnv::default();
        assert_eq!(
            mixin(&env).bettermounthud_always_render_food(&(), LivingEntity(1)),
            0
        );
    }

    #[test]
    fn air_moves_under_the_vehicle_hearts_only_while_riding_one_with_health() {
        let env = TestEnv::default();
        assert_eq!(mixin(&env).bettermounthud_move_air_up(7), 7);
        let env = TestEnv {
            vehicle: Some(LivingEntity(1)),
            vehicle_hearts: 10,
            ..Default::default()
        };
        assert_eq!(mixin(&env).bettermounthud_move_air_up(7), 10);
    }

    #[test]
    fn the_jump_bar_survives_when_experience_is_hidden_the_jump_key_is_down_or_the_meter_is_charging()
     {
        let base = TestEnv {
            has_experience: true,
            key_jump_down: false,
            jump_scale: 0.0,
            ..Default::default()
        };
        assert_eq!(mixin(&base).bettermounthud_switch_bar(PlayerRideableJumping(1)), None);
        assert_eq!(
            mixin(&TestEnv {
                has_experience: false,
                ..base.clone()
            })
            .bettermounthud_switch_bar(PlayerRideableJumping(1)),
            Some(PlayerRideableJumping(1))
        );
        assert_eq!(
            mixin(&TestEnv {
                key_jump_down: true,
                ..base.clone()
            })
            .bettermounthud_switch_bar(PlayerRideableJumping(1)),
            Some(PlayerRideableJumping(1))
        );
        assert_eq!(
            mixin(&TestEnv {
                jump_scale: 0.5,
                ..base
            })
            .bettermounthud_switch_bar(PlayerRideableJumping(1)),
            Some(PlayerRideableJumping(1))
        );
    }

    #[test]
    fn experience_level_shows_exactly_when_the_jump_bar_is_suppressed() {
        // Riding a jumpable mount with the jump key up and an idle meter:
        // the jump bar is suppressed, so the experience level renders.
        let riding = TestEnv {
            has_experience: true,
            jumpable: Some(PlayerRideableJumping(1)),
            ..Default::default()
        };
        assert!(mixin(&riding).bettermounthud_render_experience_level());
        // Jump key down: the jump bar owns the row.
        assert!(!mixin(&TestEnv {
            key_jump_down: true,
            ..riding.clone()
        })
        .bettermounthud_render_experience_level());
        // Charging meter: the jump bar owns the row.
        assert!(!mixin(&TestEnv {
            jump_scale: 0.5,
            ..riding.clone()
        })
        .bettermounthud_render_experience_level());
        // No jumpable vehicle: vanilla behavior keeps the experience level.
        assert!(mixin(&TestEnv {
            has_experience: true,
            ..Default::default()
        })
        .bettermounthud_render_experience_level());
        // No experience shown: never.
        assert!(!mixin(&TestEnv {
            jumpable: Some(PlayerRideableJumping(1)),
            ..Default::default()
        })
        .bettermounthud_render_experience_level());
    }
}
