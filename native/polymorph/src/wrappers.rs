//! Upstream `server/wrapper/IngredientWrapper.java` and
//! `server/wrapper/RecipeWrapper.java`, translated 1:1. The recipe and
//! ingredient types are host seams (`Recipe`/`RecipeHolder` collapse to the
//! id plus the ingredient list this logic reads).

use std::rc::Rc;

use crate::recipe_pair::{Identifier, Ingredient};

/// Upstream `IngredientWrapper`: wraps one ingredient for cross-recipe
/// matching.
#[derive(Clone, Debug)]
pub struct IngredientWrapper {
    ingredient: Ingredient,
}

impl IngredientWrapper {
    pub fn new(ingredient: Ingredient) -> Self {
        Self { ingredient }
    }

    pub fn get_ingredient(&self) -> &Ingredient {
        &self.ingredient
    }

    /// `matches`, line for line: null other is false, EMPTY matches only
    /// EMPTY, otherwise any stack of either side matching any stack of the
    /// other counts.
    pub fn matches(&self, other: Option<&IngredientWrapper>) -> bool {
        let Some(other) = other else {
            return false;
        };
        let other_ingredient = &other.ingredient;

        if other_ingredient == &Ingredient::EMPTY {
            return self.ingredient == Ingredient::EMPTY;
        }
        let stacks = &self.ingredient.stacks;

        for other_stack in &other_ingredient.stacks {
            for stack in stacks {
                if stack == other_stack {
                    return true;
                }
            }
        }
        false
    }
}

/// Upstream `RecipeWrapper`: a recipe holder with its ingredients wrapped.
#[derive(Clone, Debug)]
pub struct RecipeWrapper {
    id: Identifier,
    ingredients: Vec<IngredientWrapper>,
}

impl RecipeWrapper {
    /// Upstream takes `RecipeHolder<?>` and reads `id()` +
    /// `value().getIngredients()`; both arrive through the seam.
    pub fn new(id: Identifier, recipe_ingredients: Vec<Ingredient>) -> Self {
        let ingredients =
            recipe_ingredients.into_iter().map(IngredientWrapper::new).collect();
        Self { id, ingredients }
    }

    pub fn get_id(&self) -> &Identifier {
        &self.id
    }

    pub fn get_ingredients(&self) -> &[IngredientWrapper] {
        &self.ingredients
    }

    /// `conflicts`, line for line: null other is false, same id is true,
    /// different ingredient counts are false, otherwise every position must
    /// match.
    pub fn conflicts(&self, other: Option<&RecipeWrapper>) -> bool {
        let Some(other) = other else {
            return false;
        };
        if self.id == other.id {
            return true;
        }
        if self.ingredients.len() != other.ingredients.len() {
            return false;
        }
        let other_ingredients = &other.ingredients;

        for i in 0..other_ingredients.len() {
            if !other_ingredients[i].matches(Some(&self.ingredients[i])) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe_pair::ItemStack;
    use crate::recipe_pair::Identifier;

    fn stack(item: &str, count: i32) -> ItemStack {
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components: Default::default() }
    }

    fn wrapper(id: &str, ingredients: Vec<Vec<ItemStack>>) -> RecipeWrapper {
        let (ns, path) = id.split_once(':').unwrap();
        RecipeWrapper::new(
            Identifier::new(ns, path),
            ingredients
                .into_iter()
                .map(|stacks| Ingredient { stacks })
                .collect(),
        )
    }

    #[test]
    fn same_id_conflicts() {
        let a = wrapper("mod:a", vec![vec![stack("mod:stick", 1)]]);
        let b = wrapper("mod:a", vec![]);
        assert!(a.conflicts(Some(&b)));
    }

    #[test]
    fn different_sizes_do_not_conflict() {
        let a = wrapper("mod:a", vec![vec![stack("mod:stick", 1)]]);
        let b = wrapper("mod:b", vec![vec![stack("mod:stick", 1)], vec![stack("mod:stone", 1)]]);
        assert!(!a.conflicts(Some(&b)));
    }

    #[test]
    fn matching_positions_conflict() {
        let a = wrapper("mod:a", vec![vec![stack("mod:stick", 1)], vec![stack("mod:stone", 1)]]);
        let b = wrapper("mod:b", vec![vec![stack("mod:stick", 1)], vec![stack("mod:stone", 1)]]);
        assert!(a.conflicts(Some(&b)));
    }

    #[test]
    fn mismatched_position_does_not_conflict() {
        let a = wrapper("mod:a", vec![vec![stack("mod:stick", 1)], vec![stack("mod:stone", 1)]]);
        let b = wrapper("mod:b", vec![vec![stack("mod:planks", 2)], vec![stack("mod:dirt", 1)]]);
        assert!(!a.conflicts(Some(&b)));
    }

    #[test]
    fn empty_matches_only_empty() {
        let empty = IngredientWrapper::new(Ingredient::EMPTY);
        assert!(empty.matches(Some(&IngredientWrapper::new(Ingredient::EMPTY))));
        let full = IngredientWrapper::new(Ingredient { stacks: vec![stack("mod:stick", 1)] });
        assert!(!empty.matches(Some(&full)));
        assert!(!empty.matches(None));
    }
}

/// Upstream `server/wrapper/CraftingRecipeWrapper.java`. The
/// `Services.PLATFORM.isShaped` / `isSameShape` calls are platform seams
/// the caller resolves; the `RecipeWrapper` cast in the upstream
/// `conflicts` signature is a typed parameter here.
pub struct CraftingRecipeWrapper {
    inner: RecipeWrapper,
    shaped: bool,
    is_same_shape: Rc<dyn Fn(&RecipeWrapper, &RecipeWrapper) -> bool>,
}

impl CraftingRecipeWrapper {
    pub fn new(
        inner: RecipeWrapper,
        shaped: bool,
        is_same_shape: Rc<dyn Fn(&RecipeWrapper, &RecipeWrapper) -> bool>,
    ) -> Self {
        Self { inner, shaped, is_same_shape }
    }

    pub fn is_shaped(&self) -> bool {
        self.shaped
    }

    pub fn get_recipe(&self) -> &RecipeWrapper {
        &self.inner
    }

    /// `conflicts(RecipeWrapper pOther)`: the base conflict check plus the
    /// shape gate.
    pub fn conflicts(&self, other: &CraftingRecipeWrapper) -> bool {
        self.inner.conflicts(Some(&other.inner)) && self.is_same_shape(other)
    }

    /// `isSameShape`: two shaped recipes compare shapes on the platform
    /// seam; anything else counts as the same shape.
    fn is_same_shape(&self, other: &CraftingRecipeWrapper) -> bool {
        if self.shaped && other.is_shaped() {
            (self.is_same_shape)(&self.inner, &other.inner)
        } else {
            true
        }
    }
}

/// The smithing recipe parts the accessors expose on
/// `SmithingTrimRecipe` / `SmithingTransformRecipe`; a recipe that is
/// neither keeps every part `Ingredient.EMPTY`.
#[derive(Clone, Debug)]
pub struct SmithingParts {
    pub template: Ingredient,
    pub base: Ingredient,
    pub addition: Ingredient,
}

/// Upstream `server/wrapper/SmithingRecipeWrapper.java`. The accessor
/// reads are a seam closure, since the translated `Recipe` seam does not
/// carry the vanilla smithing recipe types.
pub struct SmithingRecipeWrapper {
    inner: RecipeWrapper,
    parts: Rc<dyn Fn() -> SmithingParts>,
}

impl SmithingRecipeWrapper {
    pub fn new(inner: RecipeWrapper, parts: Rc<dyn Fn() -> SmithingParts>) -> Self {
        Self { inner, parts }
    }

    pub fn get_recipe(&self) -> &RecipeWrapper {
        &self.inner
    }

    /// `conflicts(RecipeWrapper pOther)`: the base conflict check plus the
    /// three part comparisons. The upstream `&` chain is non-short-circuit
    /// and stays that way.
    pub fn conflicts(&self, other: &SmithingRecipeWrapper) -> bool {
        let mine = (self.parts)();
        let theirs = (other.parts)();
        let base_wrapper = IngredientWrapper::new(mine.base);
        let other_base_wrapper = IngredientWrapper::new(theirs.base);
        let addition_wrapper = IngredientWrapper::new(mine.addition);
        let other_addition_wrapper = IngredientWrapper::new(theirs.addition);
        let template_wrapper = IngredientWrapper::new(mine.template);
        let other_template_wrapper = IngredientWrapper::new(theirs.template);
        self.inner.conflicts(Some(&other.inner))
            & base_wrapper.matches(Some(&other_base_wrapper))
            & addition_wrapper.matches(Some(&other_addition_wrapper))
            & template_wrapper.matches(Some(&other_template_wrapper))
    }
}

#[cfg(test)]
mod crafting_and_smithing_tests {
    use super::*;
    use crate::recipe_pair::ItemStack;
    use std::rc::Rc;

    fn stack(item: &str, count: i32) -> ItemStack {
        let (ns, path) = item.split_once(':').unwrap();
        ItemStack { item: Identifier::new(ns, path), count, components: Default::default() }
    }

    fn id(s: &str) -> Identifier {
        let (ns, path) = s.split_once(':').unwrap();
        Identifier::new(ns, path)
    }

    /// The platform `isSameShape` seam is global upstream, so the tests
    /// share one closure whose verdict a Cell flips.
    fn crafting(id_str: &str, shaped: bool, platform: Rc<std::cell::Cell<bool>>) -> CraftingRecipeWrapper {
        CraftingRecipeWrapper::new(
            RecipeWrapper::new(id(id_str), vec![Ingredient { stacks: vec![stack("test:plank", 1)] }]),
            shaped,
            Rc::new(move |_, _| platform.get()),
        )
    }

    #[test]
    fn crafting_conflicts_need_the_same_shape() {
        let platform = Rc::new(std::cell::Cell::new(true));
        let a = crafting("test:a", true, platform.clone());
        let b = crafting("test:b", true, platform.clone());
        assert!(a.conflicts(&b));
        platform.set(false);
        let c = crafting("test:c", true, platform.clone());
        assert!(!a.conflicts(&c), "different shapes do not conflict");
    }

    #[test]
    fn the_shape_gate_only_runs_between_two_shaped_recipes() {
        let platform = Rc::new(std::cell::Cell::new(false));
        let a = crafting("test:a", false, platform.clone());
        let b = crafting("test:b", false, platform.clone());
        assert!(a.conflicts(&b), "the platform shape check never runs unshaped");
        let shaped = crafting("test:c", true, platform.clone());
        assert!(a.conflicts(&shaped), "either side unshaped counts as the same shape");
    }

    fn smithing(
        id_str: &str,
        template: Option<&str>,
        base: Option<&str>,
        addition: Option<&str>,
    ) -> SmithingRecipeWrapper {
        let parts = SmithingParts {
            template: Ingredient {
                stacks: template.map(|t| vec![stack(t, 1)]).unwrap_or_default(),
            },
            base: Ingredient {
                stacks: base.map(|b| vec![stack(b, 1)]).unwrap_or_default(),
            },
            addition: Ingredient {
                stacks: addition.map(|a| vec![stack(a, 1)]).unwrap_or_default(),
            },
        };
        SmithingRecipeWrapper::new(
            RecipeWrapper::new(id(id_str), vec![Ingredient::EMPTY]),
            Rc::new(move || parts.clone()),
        )
    }

    #[test]
    fn smithing_conflicts_match_every_part() {
        let a = smithing("test:a", Some("test:t"), Some("test:b"), Some("test:a1"));
        let b = smithing("test:b", Some("test:t"), Some("test:b"), Some("test:a1"));
        assert!(a.conflicts(&b));
        let c = smithing("test:c", Some("test:t"), Some("test:b"), Some("test:other"));
        assert!(!a.conflicts(&c), "a different addition does not conflict");
    }

    #[test]
    fn smithing_empty_parts_only_match_empty() {
        let plain = smithing("test:plain", None, None, None);
        let trim = smithing("test:trim", Some("test:t"), Some("test:b"), Some("test:a1"));
        assert!(!plain.conflicts(&trim), "EMPTY never matches a filled part");
        assert!(plain.conflicts(&smithing("test:plain2", None, None, None)));
    }
}
