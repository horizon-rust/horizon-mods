//! nl.enjarai.showmeyourskin.config.HideableEquipment (1:1).

use serde_json::Value;

use crate::seams::EquipmentSlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HideableEquipment {
    Head,
    Chest,
    Legs,
    Feet,
    Elytra,
    Shield,
    Hat,
}

impl HideableEquipment {
    /// Java: this.id = name().toLowerCase().
    pub fn get_id(self) -> &'static str {
        match self {
            HideableEquipment::Head => "head",
            HideableEquipment::Chest => "chest",
            HideableEquipment::Legs => "legs",
            HideableEquipment::Feet => "feet",
            HideableEquipment::Elytra => "elytra",
            HideableEquipment::Shield => "shield",
            HideableEquipment::Hat => "hat",
        }
    }

    /// Java: fromId(id), null when no value matches.
    pub fn from_id(id: &str) -> Option<HideableEquipment> {
        match id {
            "head" => Some(HideableEquipment::Head),
            "chest" => Some(HideableEquipment::Chest),
            "legs" => Some(HideableEquipment::Legs),
            "feet" => Some(HideableEquipment::Feet),
            "elytra" => Some(HideableEquipment::Elytra),
            "shield" => Some(HideableEquipment::Shield),
            "hat" => Some(HideableEquipment::Hat),
            _ => None,
        }
    }

    /// Java: fromSlot(slot), null on MAINHAND/OFFHAND/BODY/SADDLE.
    pub fn from_slot(slot: EquipmentSlot) -> Option<HideableEquipment> {
        match slot {
            EquipmentSlot::Head => Some(HideableEquipment::Head),
            EquipmentSlot::Chest => Some(HideableEquipment::Chest),
            EquipmentSlot::Legs => Some(HideableEquipment::Legs),
            EquipmentSlot::Feet => Some(HideableEquipment::Feet),
            _ => None,
        }
    }

    /// Java: toSlot(), null on ELYTRA/SHIELD/HAT.
    pub fn to_slot(self) -> Option<EquipmentSlot> {
        match self {
            HideableEquipment::Head => Some(EquipmentSlot::Head),
            HideableEquipment::Chest => Some(EquipmentSlot::Chest),
            HideableEquipment::Legs => Some(EquipmentSlot::Legs),
            HideableEquipment::Feet => Some(EquipmentSlot::Feet),
            _ => None,
        }
    }

    /// Java: Endec.STRING.xmap(HideableEquipment::fromId, HideableEquipment::getId).
    pub fn encode(self) -> Value {
        Value::String(self.get_id().to_string())
    }

    pub fn decode(v: &Value) -> Option<HideableEquipment> {
        v.as_str().and_then(HideableEquipment::from_id)
    }
}

/// Java: Endec.STRING.xmap(EquipmentSlot::byName, EquipmentSlot::getName).
pub fn encode_slot(slot: EquipmentSlot) -> Value {
    Value::String(slot.get_name().to_string())
}

pub fn decode_slot(v: &Value) -> Option<EquipmentSlot> {
    v.as_str().and_then(EquipmentSlot::by_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for e in [
            HideableEquipment::Head,
            HideableEquipment::Chest,
            HideableEquipment::Legs,
            HideableEquipment::Feet,
            HideableEquipment::Elytra,
            HideableEquipment::Shield,
            HideableEquipment::Hat,
        ] {
            assert_eq!(HideableEquipment::from_id(e.get_id()), Some(e));
        }
        assert_eq!(HideableEquipment::from_id("torso"), None);
    }

    #[test]
    fn slot_matrix() {
        assert_eq!(
            HideableEquipment::from_slot(EquipmentSlot::Head),
            Some(HideableEquipment::Head)
        );
        assert_eq!(HideableEquipment::from_slot(EquipmentSlot::MainHand), None);
        assert_eq!(HideableEquipment::Head.to_slot(), Some(EquipmentSlot::Head));
        assert_eq!(HideableEquipment::Elytra.to_slot(), None);
        assert_eq!(
            decode_slot(&encode_slot(EquipmentSlot::OffHand)),
            Some(EquipmentSlot::OffHand)
        );
    }
}
