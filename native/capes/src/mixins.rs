//! Upstream client mixins, 1:1: `MixinPlayerInfo.java`,
//! `MixinCapeFeatureRenderer.java`, `MixinEntityRenderDispatcher.java`,
//! `AccessorEntityRenderDispatcher.java`, `MixinSkinCustomizationScreen.java`.
/*
 * Copyright (C) CaelTheColher and contributors.
 * Licensed under the GNU Lesser General Public License v2.1.
 */

use crate::config::{CapeConfig, CapeType};
use crate::capes_mod::Capes;
use crate::handler::{GameProfile, Identifier, PlayerHandler, Texture, Uuid};
use crate::menus::{skin_customization_icon_button, skin_customization_icon_clicked, Button, ScreenHost};
use crate::render::{PlayerSkin, PlaceholderEntityRenderState};

/// The `PlayerInfo` fields the mixin shadows (`@Shadow @Final profile`) and
/// the skin supplier it returns.
pub trait PlayerInfoHost {
    fn profile(&self) -> GameProfile;
    /// `getSkin()`'s current return value.
    fn skin(&self) -> PlayerSkin;
    /// `cir.setReturnValue(...)`.
    fn set_return(&mut self, skin: PlayerSkin);
}

/// `MixinPlayerInfo#loadTextures` (`createSkinLookup` HEAD inject): touch the
/// handler when the profile is valid.
pub fn loadTextures(host: &dyn PlayerInfoHost) {
    if crate::util::is_valid_profile(&host.profile()) {
        PlayerHandler::on_load_texture(&host.profile());
    }
}

/// `MixinPlayerInfo#getCapeTexture` (`getSkin` TAIL, cancellable): swap in the
/// loaded cape and elytra textures when the handler has a cape.
pub fn get_cape_texture(host: &mut dyn PlayerInfoHost) {
    if !crate::util::is_valid_profile(&host.profile()) {
        return;
    }
    let handler = PlayerHandler::from_profile(&host.profile());
    let has_cape = handler.borrow().has_cape;
    if has_cape {
        let config: CapeConfig = Capes::config();
        let old_textures = host.skin();
        let cape_texture = handler.borrow_mut().get_cape();
        let elytra_texture = if handler.borrow().has_elytra_texture
            && config.enable_elytra_texture
        {
            cape_texture.clone()
        } else {
            Texture::ResourceTexture(Identifier::parse(
                "textures/entity/equipment/wings/elytra.png",
            ))
        };
        host.set_return(PlayerSkin {
            cape: Some(cape_texture),
            elytra: elytra_texture,
            ..old_textures
        });
    }
}

/// `MixinCapeFeatureRenderer#fixCapeTransparency` (`submit*` WrapOperation on
/// `RenderTypes.entitySolid`): return `armorCutoutNoCull` instead.
pub fn fix_cape_transparency(original: RenderKind, texture: Identifier) -> RenderKind {
    let _ = texture;
    match original {
        RenderKind::EntitySolid => RenderKind::ArmorCutoutNoCull,
        other => other,
    }
}

/// The `RenderType` factories the wrap can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderKind {
    EntitySolid,
    ArmorCutoutNoCull,
}

/// `MixinEntityRenderDispatcher#getPlaceholderRenderer` (`getRenderer` HEAD,
/// cancellable): hand back the placeholder renderer for its render state.
pub fn get_placeholder_renderer(state: &dyn std::any::Any) -> Option<()> {
    if state.is::<PlaceholderEntityRenderState>() {
        Some(())
    } else {
        None
    }
}

/// `AccessorEntityRenderDispatcher` as the read seam the placeholder init
/// uses.
pub fn accessor_entity_render_dispatcher(
    equipment_assets: u64,
    block_model_resolver: u64,
) -> (u64, u64) {
    (equipment_assets, block_model_resolver)
}

/// `MixinSkinCustomizationScreen#addOptions` RETURN injection: add the sprite
/// icon button and wire it to open the selector menu.
pub fn skin_customization_add_options(host: &dyn ScreenHost, widgets: &mut Vec<Button>) {
    widgets.push(skin_customization_icon_button(host));
}

/// The icon button's click handler, per the mixin's builder callback.
pub fn skin_customization_icon_on_click(host: &mut dyn ScreenHost) {
    skin_customization_icon_clicked(host);
}

/// Keep the CapeType import used for the toggle text paths exercised through
/// the config.
#[allow(dead_code)]
fn unused_cape_type_cycle(t: CapeType) -> CapeType {
    t.cycle()
}

/// Keep the UUID import tied to the profile shape the mixin reads.
#[allow(dead_code)]
fn unused_uuid(u: Uuid) -> Uuid {
    u
}
