//! nl.enjarai.showmeyourskin.pond (1:1): the ArmorConfigContraband duck
//! interface the mannequin mixin implements.

use crate::config::armor_config::ArmorConfig;

/// Java: pond/ArmorConfigContraband (show_me_your_skin$-prefixed duck typing).
pub trait ArmorConfigContraband {
    fn show_me_your_skin_set_armor_config(&mut self, armor_config: Option<ArmorConfig>);
    fn show_me_your_skin_get_armor_config(&self) -> Option<ArmorConfig>;
}

/// Java: mixin/ClientMannequinMixin - the @Unique armorConfig field backing
/// the contraband interface on ClientMannequin.
#[derive(Default)]
pub struct ClientMannequinContraband {
    armor_config: Option<ArmorConfig>,
}

impl ArmorConfigContraband for ClientMannequinContraband {
    fn show_me_your_skin_set_armor_config(&mut self, armor_config: Option<ArmorConfig>) {
        self.armor_config = armor_config;
    }

    fn show_me_your_skin_get_armor_config(&self) -> Option<ArmorConfig> {
        self.armor_config.clone()
    }
}

/// Java: mixin/ClientMannequinAccessor - @Invoker invokeUpdateSkin().
pub fn invoke_update_skin() {
    // Seamed: ClientMannequin.updateSkin() private method call.
}
