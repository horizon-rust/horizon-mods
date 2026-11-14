//! Upstream `common/util/RecipePair.java` (record implementing IRecipePair),
//! translated 1:1. The Minecraft registry, `ItemStack` and
//! `DataComponentMap` are host seams with the same shapes and semantics.

use std::collections::BTreeMap;

/// Host seam for `net.minecraft.resources.ResourceLocation` (namespace:path).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub namespace: String,
    pub path: String,
}

impl Identifier {
    pub fn new(namespace: &str, path: &str) -> Self {
        Self { namespace: namespace.to_string(), path: path.to_string() }
    }

    /// `ResourceLocation#compareTo`: namespace, then path (host seam).
    pub fn compare_to(&self, other: &Identifier) -> std::cmp::Ordering {
        self.namespace
            .cmp(&other.namespace)
            .then(self.path.cmp(&other.path))
    }
}

/// A stable 32-bit component hash for the seam map (the Java code only ever
/// subtracts two of these, never stores them).
fn component_hash(map: &BTreeMap<String, String>) -> i32 {
    let mut h: u32 = 2166136261;
    for (k, v) in map {
        for b in k.bytes().chain(std::iter::once(0x1f)).chain(v.bytes()) {
            h = h.wrapping_mul(16777619) ^ b as u32;
        }
    }
    h as i32
}

/// Host seam for `net.minecraft.world.item.ItemStack` reduced to what the
/// translated logic reads: the item's registry key, the count, and the
/// component map (`ItemStack#matches` semantics). Hash supports the wire
/// HashSet in the network packets.
#[derive(Clone, Debug, Hash)]
pub struct ItemStack {
    pub item: Identifier,
    pub count: i32,
    pub components: BTreeMap<String, String>,
}

impl PartialEq for ItemStack {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
            && self.count == other.count
            && self.components == other.components
    }
}

/// Host seam for `net.minecraft.world.item.crafting.Ingredient`; EMPTY is the
/// empty ingredient (no accepted stacks).
#[derive(Clone, Debug, PartialEq)]
pub struct Ingredient {
    pub stacks: Vec<ItemStack>,
}

impl Ingredient {
    pub const EMPTY: Ingredient = Ingredient { stacks: Vec::new() };
}

/// Upstream record `RecipePair(ResourceLocation resourceLocation,
/// ItemStack output) implements IRecipePair`. Hash supports the wire
/// HashSet in the network packets.
#[derive(Clone, Debug, Hash)]
pub struct RecipePair {
    pub resource_location: Identifier,
    pub output: ItemStack,
}

impl ItemStack {
    /// Host seam for `ItemStack#isEmpty` (count 0 is the empty stack).
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Host seam for `ItemStack.isSameItemSameComponents`: same registry key
    /// and same component map, count-insensitive.
    pub fn is_same_item_same_components(a: &ItemStack, b: &ItemStack) -> bool {
        a.item == b.item && a.components == b.components
    }
}

impl RecipePair {
    /// `IRecipePair#compareTo`, line for line: item registry key, then count,
    /// then component-map equality/hash, with vanilla recipes sorted after
    /// modded ones when they share the list.
    pub fn compare_to(&self, other: &RecipePair) -> i32 {
        let output1 = &self.output;
        let output2 = &other.output;
        let compare = output1.item.compare_to(&output2.item);

        if compare.is_eq() {
            let compare = output1.count - output2.count;

            if compare == 0 {
                let components1 = &output1.components;
                let components2 = &output2.components;

                if components1 == components2 {
                    0
                } else {
                    component_hash(components1) - component_hash(components2)
                }
            } else {
                compare
            }
        } else {
            // Sort vanilla recipes after modded recipes if they appear in the same list
            if self.resource_location.namespace == "minecraft"
                && other.resource_location.namespace != "minecraft"
            {
                1
            } else if self.resource_location.namespace != "minecraft"
                && other.resource_location.namespace == "minecraft"
            {
                -1
            } else {
                compare as i32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &str, count: i32, tag: &str) -> ItemStack {
        let mut components = BTreeMap::new();
        if !tag.is_empty() {
            components.insert("tag".to_string(), tag.to_string());
        }
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components }
    }

    fn pair(id: &str, item: &str, count: i32, tag: &str) -> RecipePair {
        let (ns, path) = id.split_once(':').unwrap();
        RecipePair { resource_location: Identifier::new(ns, path), output: stack(item, count, tag) }
    }

    #[test]
    fn same_item_orders_by_count() {
        let a = pair("mod:a", "mod:out", 2, "");
        let b = pair("mod:b", "mod:out", 1, "");
        assert!(a.compare_to(&b) > 0);
        assert!(b.compare_to(&a) < 0);
    }

    #[test]
    fn same_output_components_break_ties() {
        let a = pair("mod:a", "mod:out", 1, "x");
        let b = pair("mod:b", "mod:out", 1, "y");
        let c = pair("mod:b", "mod:out", 1, "x");
        assert_eq!(a.compare_to(&c), 0);
        assert_ne!(a.compare_to(&b), 0);
        assert_eq!(b.compare_to(&a), -(a.compare_to(&b)));
    }

    #[test]
    fn vanilla_sorts_after_modded() {
        let vanilla = pair("minecraft:stick", "mod:out1", 1, "");
        let modded = pair("mod:x", "mod:out2", 1, "");
        assert_eq!(vanilla.compare_to(&modded), 1);
        assert_eq!(modded.compare_to(&vanilla), -1);
    }

    #[test]
    fn two_modded_fall_through_to_item_compare() {
        let a = pair("mod:aa", "mod:out1", 1, "");
        let b = pair("mod:bb", "mod:out2", 1, "");
        assert_eq!(a.compare_to(&b) < 0, true);
    }
}

impl Ord for RecipePair {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let c = self.compare_to(other);
        if c < 0 { std::cmp::Ordering::Less } else if c > 0 { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }
    }
}

impl PartialOrd for RecipePair {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for RecipePair {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for RecipePair {}
