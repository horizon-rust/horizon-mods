//! Upstream `api/client/base/*`, `api/client/widgets/**`,
//! `client/RecipesWidget.java`, `client/PolymorphWidgetsImpl.java` and
//! `client/PolymorphClientEvents.java`, translated 1:1. The Minecraft client
//! GUI (`GuiGraphics`, `AbstractWidget`, `ImageButton`, `WidgetSprites`),
//! `AbstractContainerScreen`/`AbstractContainerMenu`, `Minecraft` and the
//! C2S send path are host seams with the same shapes and semantics. Java
//! inheritance becomes a behavior enum: the three widget subclasses differ
//! only in which output slot they bind and how `selectRecipe` routes.

use crate::network::MOD_ID;
use crate::recipe_pair::{Identifier, ItemStack, RecipePair};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

/// Host seam for `net.minecraft.client.gui.components.WidgetSprites`.
#[derive(Clone, Debug)]
pub struct WidgetSprites {
    /// `sprite.enabled()`: the idle texture.
    pub enabled: Identifier,
    /// `sprite.enabledFocused()`: the hovered texture.
    pub enabled_focused: Identifier,
}

/// Host seam for `Pair<WidgetSprites, WidgetSprites>`.
#[derive(Clone, Debug)]
pub struct SpritePair {
    pub first: WidgetSprites,
    pub second: WidgetSprites,
}

/// `AbstractRecipesWidget.OUTPUT`.
pub fn output_sprites() -> WidgetSprites {
    WidgetSprites {
        enabled: Identifier::new(MOD_ID, "output_button"),
        enabled_focused: Identifier::new(MOD_ID, "output_button_highlighted"),
    }
}

/// `AbstractRecipesWidget.CURRENT_OUTPUT`.
pub fn current_output_sprites() -> WidgetSprites {
    WidgetSprites {
        enabled: Identifier::new(MOD_ID, "current_output"),
        enabled_focused: Identifier::new(MOD_ID, "current_output_highlighted"),
    }
}

/// `AbstractRecipesWidget.SELECTOR`.
pub fn selector_sprites() -> WidgetSprites {
    WidgetSprites {
        enabled: Identifier::new(MOD_ID, "selector_button"),
        enabled_focused: Identifier::new(MOD_ID, "selector_button_highlighted"),
    }
}

pub const BUTTON_X_OFFSET: i32 = 0;
pub const BUTTON_Y_OFFSET: i32 = -22;
pub const WIDGET_X_OFFSET: i32 = -4;
pub const WIDGET_Y_OFFSET: i32 = -26;

/// Host seam for `net.minecraft.world.inventory.Slot` reduced to what the
/// widgets read: the draw position and the backing container kind, which
/// drives `findResultSlot` (`ResultContainer`) and the crafter scan
/// (`NonInteractiveResultSlot`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotRef {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub container: ContainerKind,
}

/// The backing `Container` classes the widget layer distinguishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    /// `ResultContainer`.
    Result,
    /// `NonInteractiveResultSlot`'s container.
    NonInteractiveResult,
    /// Any other container.
    Other,
}

/// Host seam for `AbstractContainerScreen<?>` and its menu: identity, the
/// slot list, the screen's draw origin and the menu type, which drives
/// `PolymorphWidgetsImpl.setup`'s instanceof chain.
pub trait ClientScreen {
    /// Screen identity; two calls with the same id are one screen.
    fn id(&self) -> u64;
    /// `containerScreen.getMenu().slots`.
    fn menu_slots(&self) -> Vec<SlotRef>;
    /// `Services.CLIENT_PLATFORM.getScreenLeft(screen)`.
    fn screen_left(&self) -> i32;
    /// `Services.CLIENT_PLATFORM.getScreenTop(screen)`.
    fn screen_top(&self) -> i32;
    /// The menu kind for the `setup` matcher.
    fn menu_kind(&self) -> MenuKind;
}

/// The menu classes `PolymorphWidgetsImpl.setup` matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    /// `SmithingMenu`.
    Smithing,
    /// `AbstractFurnaceMenu`.
    AbstractFurnace,
    /// `CrafterMenu`.
    Crafter,
    /// Any other menu.
    Other,
}

/// Host seam for the client side of `GuiGraphics`.
pub trait GuiGraphics {
    /// `guiGraphics.blitSprite(texture, x, y, z, width, height)`.
    fn blit_sprite(&mut self, texture: &Identifier, x: i32, y: i32, z: i32, width: i32, height: i32);
    /// `guiGraphics.renderItem(stack, x, y)`.
    fn render_item(&mut self, stack: &ItemStack, x: i32, y: i32);
    /// `guiGraphics.renderItemDecorations(font, stack, x, y)`.
    fn render_item_decorations(&mut self, stack: &ItemStack, x: i32, y: i32);
    /// `guiGraphics.renderTooltip(font, stack, mouseX, mouseY)`.
    fn render_tooltip(&mut self, stack: &ItemStack, mouse_x: i32, mouse_y: i32);
}

/// Host seam for the client world/network face the widgets touch:
/// `Minecraft.getInstance()` connection and player, the level recipe
/// manager's `byKey`, and the three C2S sends on
/// `PolymorphApi.getInstance().getNetwork()`.
pub trait ClientWidgetEnv {
    /// `Minecraft.getInstance().getConnection() != null`.
    fn connection_present(&self) -> bool;
    /// `level.getRecipeManager().byKey(id)` on the client player's level.
    fn recipe_by_key(&self, id: &Identifier) -> Option<crate::recipe_cache::RecipeHolder>;
    /// `api.getPlayerRecipeData(player).selectRecipe(recipe)`.
    fn select_on_player_capability(&mut self, recipe: crate::recipe_cache::RecipeHolder);
    /// `sendBlockEntityListenerC2S(add)`.
    fn send_block_entity_listener_c2s(&mut self, add: bool);
    /// `sendPersistentRecipeSelectionC2S(resourceLocation)`.
    fn send_persistent_recipe_selection_c2s(&mut self, id: &Identifier);
    /// `sendPlayerRecipeSelectionC2S(resourceLocation)`.
    fn send_player_recipe_selection_c2s(&mut self, id: &Identifier);
}

/// `api/client/widgets/children/OutputWidget.java`.
pub struct OutputWidget {
    x: i32,
    y: i32,
    /// `AbstractWidget.visible`.
    pub visible: bool,
    output: ItemStack,
    resource_location: Identifier,
    sprites: SpritePair,
    highlighted: bool,
}

impl OutputWidget {
    pub fn new(sprites: SpritePair, recipe_pair: &RecipePair) -> Self {
        Self {
            x: 0,
            y: 0,
            visible: true,
            output: recipe_pair.output.clone(),
            resource_location: recipe_pair.resource_location.clone(),
            sprites,
            highlighted: false,
        }
    }

    /// `setPosition` (AbstractWidget host seam).
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn get_x(&self) -> i32 {
        self.x
    }

    pub fn get_y(&self) -> i32 {
        self.y
    }

    /// `getWidth()` is pinned to 25.
    pub fn get_width(&self) -> i32 {
        25
    }

    pub fn get_output(&self) -> &ItemStack {
        &self.output
    }

    pub fn get_resource_location(&self) -> &Identifier {
        &self.resource_location
    }

    pub fn set_highlighted(&mut self, highlighted: bool) {
        self.highlighted = highlighted;
    }

    pub fn is_highlighted(&self) -> bool {
        self.highlighted
    }

    /// `renderWidget`: the sprite pair's second half when highlighted, the
    /// focused texture on hover, the sprite blitted at z=600, the item and
    /// its decorations pushed to z=700.
    pub fn render_widget(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        let sprite = if self.highlighted {
            &self.sprites.second
        } else {
            &self.sprites.first
        };
        let mut texture = sprite.enabled.clone();

        if self.x + 25 > mouse_x && self.x <= mouse_x && self.y + 25 > mouse_y && self.y <= mouse_y
        {
            texture = sprite.enabled_focused.clone();
        }
        gui.blit_sprite(&texture, self.x, self.y, 600, self.get_width(), self.get_width());
        let k = 4;
        gui.render_item(&self.output, self.x + k, self.y + k);
        gui.render_item_decorations(&self.output, self.x + k, self.y + k);
    }

    /// `isValidClickButton`: button 0 or 1.
    fn is_valid_click_button(&self, button: i32) -> bool {
        button == 0 || button == 1
    }

    /// `AbstractWidget#mouseClicked` on this 25x25 widget.
    pub fn mouse_clicked(&self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        let inside = (mouse_x as i32) > self.x
            && (mouse_x as i32) < self.x + 25
            && (mouse_y as i32) > self.y
            && (mouse_y as i32) < self.y + 25;
        inside && self.is_valid_click_button(button)
    }
}

/// `api/client/widgets/children/SelectionWidget.java`.
pub struct SelectionWidget {
    on_select: Rc<dyn Fn(&Identifier)>, // upstream `onSelect`
    container_screen: Rc<dyn ClientScreen>,
    output_widgets: Vec<OutputWidget>,
    sprites: SpritePair,
    x_offset: i32,
    y_offset: i32,
    /// `hoveredButton`, as an index into `output_widgets`.
    hovered_button: Option<usize>,
    active: bool,
    x: i32,
    y: i32,
    last_x: i32,
    last_y: i32,
}

impl SelectionWidget {
    pub fn new(
        x: i32,
        y: i32,
        x_offset: i32,
        y_offset: i32,
        sprites: SpritePair,
        on_select: Rc<dyn Fn(&Identifier)>,
        container_screen: Rc<dyn ClientScreen>,
    ) -> Self {
        let mut widget = Self {
            on_select,
            container_screen,
            output_widgets: Vec::new(),
            sprites,
            x_offset,
            y_offset,
            hovered_button: None,
            active: false,
            x: 0,
            y: 0,
            last_x: -1,
            last_y: -1,
        };
        widget.set_position(x, y);
        widget
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
        self.update_button_positions();
    }

    pub fn set_offsets(&mut self, x: i32, y: i32) {
        self.x_offset = x;
        self.y_offset = y;
    }

    /// `highlightButton`: highlight only the widget with that recipe.
    pub fn highlight_button(&mut self, resource_location: &Identifier) {
        for widget in &mut self.output_widgets {
            widget.set_highlighted(widget.get_resource_location() == resource_location);
        }
    }

    /// `updateButtonPositions`: center the row on the anchor, with the even
    /// count nudged 13 right.
    fn update_button_positions(&mut self) {
        let size = self.output_widgets.len() as i32;
        let mut x_offset = -25 * (size as f32 / 2.0).floor() as i32;

        if size % 2 == 0 {
            x_offset += 13;
        }
        let mut pos = (self.x + x_offset, self.y);
        for widget in &mut self.output_widgets {
            widget.set_position(pos.0, pos.1);
            pos.0 += 25;
        }
    }

    pub fn get_output_widgets(&self) -> &[OutputWidget] {
        &self.output_widgets
    }

    /// `setRecipeList`: drop pairs whose output is empty, then re-layout.
    pub fn set_recipe_list(&mut self, recipe_list: &BTreeSet<RecipePair>) {
        self.output_widgets.clear();
        for data in recipe_list {
            if !data.output.is_empty() {
                self.output_widgets.push(OutputWidget::new(self.sprites.clone(), data));
            }
        }
        self.update_button_positions();
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// `renderTooltip`: the hovered button's output at z=501.
    fn render_tooltip(&self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        if let Some(index) = self.hovered_button {
            let button = &self.output_widgets[index];
            gui.render_tooltip(button.get_output(), mouse_x, mouse_y);
        }
    }

    /// `render`: when active, track the screen's draw origin, render each
    /// button and record the hovered one.
    pub fn render(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        if self.is_active() {
            let x = self.container_screen.screen_left() + self.x_offset;
            let y = self.container_screen.screen_top() + self.y_offset;

            if self.last_x != x || self.last_y != y {
                self.set_position(x, y);
                self.last_x = x;
                self.last_y = y;
            }
            self.hovered_button = None;
            for (index, button) in self.output_widgets.iter_mut().enumerate() {
                button.render_widget(gui, mouse_x, mouse_y);

                if button.visible && button.mouse_clicked(mouse_x as f64, mouse_y as f64, 0) {
                    self.hovered_button = Some(index);
                }
            }
            self.render_tooltip(gui, mouse_x, mouse_y);
        }
    }

    /// `mouseClicked`: fire `onSelect` for the clicked widget.
    pub fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        if self.is_active() {
            for widget in &self.output_widgets {
                if widget.mouse_clicked(mouse_x, mouse_y, button) {
                    (self.on_select)(widget.get_resource_location());
                    return true;
                }
            }
        }
        false
    }
}

/// `api/client/widgets/children/OpenSelectionButton.java`, an `ImageButton`
/// 16x16 that repositions itself against the screen's draw origin on render.
pub struct OpenSelectionButton {
    x: i32,
    y: i32,
    container_screen: Rc<dyn ClientScreen>,
    x_offset: i32,
    y_offset: i32,
    sprites: WidgetSprites,
    /// The `OnPress` callback.
    pub on_press: Rc<dyn Fn(&mut OpenSelectionButton)>,
    /// `AbstractWidget.visible`.
    pub visible: bool,
    active: bool,
}

impl OpenSelectionButton {
    pub fn new(
        container_screen: Rc<dyn ClientScreen>,
        x: i32,
        y: i32,
        sprites: WidgetSprites,
        on_press: Rc<dyn Fn(&mut OpenSelectionButton)>,
    ) -> Self {
        Self {
            x: 0,
            y: 0,
            container_screen,
            x_offset: x,
            y_offset: y,
            sprites,
            on_press,
            visible: true,
            active: true,
        }
    }

    pub fn set_offsets(&mut self, x: i32, y: i32) {
        self.x_offset = x;
        self.y_offset = y;
    }

    pub fn get_x(&self) -> i32 {
        self.x
    }

    pub fn get_y(&self) -> i32 {
        self.y
    }

    /// `renderWidget`: reset the shader color, reposition against the
    /// screen's draw origin, then the super render (the sprite blit).
    pub fn render_widget(&mut self, gui: &mut dyn GuiGraphics, _mouse_x: i32, _mouse_y: i32) {
        self.x = self.container_screen.screen_left() + self.x_offset;
        self.y = self.container_screen.screen_top() + self.y_offset;
        let texture = if self.active {
            self.sprites.enabled.clone()
        } else {
            self.sprites.enabled_focused.clone()
        };
        gui.blit_sprite(&texture, self.x, self.y, 0, 16, 16);
    }

    /// `AbstractWidget#mouseClicked` on this 16x16 button, then `onPress`.
    pub fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        let inside = (mouse_x as i32) >= self.x
            && (mouse_x as i32) < self.x + 16
            && (mouse_y as i32) >= self.y
            && (mouse_y as i32) < self.y + 16;
        if inside && button == 0 {
            let on_press = self.on_press.clone();
            on_press(self);
            true
        } else {
            false
        }
    }
}

/// Which upstream subclass the widget stands for; they differ only in their
/// output slot and `selectRecipe` routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetBehavior {
    /// `PersistentRecipesWidget`: `selectRecipe` sends the persistent C2S.
    Persistent,
    /// `PlayerRecipesWidget`: local capability select, then the player C2S.
    Player,
}

/// `api/client/base/AbstractRecipesWidget.java` with the subclass split
/// carried in `behavior` and `output_slot`.
pub struct AbstractRecipesWidget {
    pub container_screen: Rc<dyn ClientScreen>,
    env: Rc<RefCell<dyn ClientWidgetEnv>>,
    pub x_offset: i32,
    pub y_offset: i32,
    pub behavior: WidgetBehavior,
    /// The bound output slot (`FurnaceRecipesWidget` slot 2, the crafter
    /// scan, or the passed-in `PlayerRecipesWidget` slot).
    pub output_slot: SlotRef,
    pub selection_widget: Option<SelectionWidget>,
    pub open_button: Option<OpenSelectionButton>,
}

impl AbstractRecipesWidget {
    /// `AbstractRecipesWidget(containerScreen)`: the default offsets.
    pub fn new(
        container_screen: Rc<dyn ClientScreen>,
        env: Rc<RefCell<dyn ClientWidgetEnv>>,
        behavior: WidgetBehavior,
        output_slot: SlotRef,
    ) -> Self {
        Self {
            container_screen,
            env,
            x_offset: WIDGET_X_OFFSET,
            y_offset: WIDGET_Y_OFFSET,
            behavior,
            output_slot,
            selection_widget: None,
            open_button: None,
        }
    }

    /// `FurnaceRecipesWidget`: the furnace's result slot is index 2.
    pub fn new_furnace(
        container_screen: Rc<dyn ClientScreen>,
        env: Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Self {
        let slot = container_screen
            .menu_slots()
            .get(2)
            .copied()
            .expect("furnace menu slot 2");
        Self::new(container_screen, env, WidgetBehavior::Persistent, slot)
    }

    /// `CrafterRecipesWidget`: the first `NonInteractiveResultSlot`, else
    /// the menu's first slot.
    pub fn new_crafter(
        container_screen: Rc<dyn ClientScreen>,
        env: Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Self {
        let slots = container_screen.menu_slots();
        let slot = slots
            .iter()
            .find(|slot| slot.container == ContainerKind::NonInteractiveResult)
            .or_else(|| slots.first())
            .copied()
            .expect("crafter menu slot");
        Self::new(container_screen, env, WidgetBehavior::Persistent, slot)
    }

    /// `getSelectorSprites`.
    pub fn get_selector_sprites(&self) -> WidgetSprites {
        selector_sprites()
    }

    /// `getOutputSprites`.
    pub fn get_output_sprites(&self) -> SpritePair {
        SpritePair { first: output_sprites(), second: current_output_sprites() }
    }

    /// `resetWidgetOffsets`.
    pub fn reset_widget_offsets(&mut self) {
        let x = self.get_x_pos();
        let y = self.get_y_pos();
        if let Some(widget) = &mut self.selection_widget {
            widget.set_offsets(x + self.x_offset, y + self.y_offset);
        }
        if let Some(button) = &mut self.open_button {
            button.set_offsets(x, y);
        }
    }

    /// `initChildWidgets`.
    pub fn init_child_widgets(&mut self) {
        let x = self.container_screen.screen_left() + self.get_x_pos();
        let y = self.container_screen.screen_top() + self.get_y_pos();
        let env = self.env.clone();
        let behavior = self.behavior;
        let screen = self.container_screen.clone();
        let sprites = self.get_output_sprites();
        let selection_widget = SelectionWidget::new(
            x + self.x_offset,
            y + self.y_offset,
            self.get_x_pos() + self.x_offset,
            self.get_y_pos() + self.y_offset,
            sprites,
            Rc::new(move |id: &Identifier| match behavior {
                WidgetBehavior::Persistent => {
                    env.borrow_mut().send_persistent_recipe_selection_c2s(id);
                }
                WidgetBehavior::Player => {
                    // `PlayerRecipesWidget.selectRecipe`: the local
                    // capability select through the recipe manager, then the
                    // C2S packet.
                    let recipe = env.borrow().recipe_by_key(id);
                    if let Some(recipe) = recipe {
                        env.borrow_mut().select_on_player_capability(recipe);
                    }
                    env.borrow_mut().send_player_recipe_selection_c2s(id);
                }
            }),
            screen,
        );
        let visible_more_than_one = selection_widget.get_output_widgets().len() > 1;
        let screen = self.container_screen.clone();
        let x_pos = self.get_x_pos();
        let y_pos = self.get_y_pos();
        let selector = self.get_selector_sprites();
        let open_button = OpenSelectionButton::new(
            screen,
            x_pos,
            y_pos,
            selector,
            Rc::new(|_button: &mut OpenSelectionButton| {
                // The toggle itself runs at the widget level below, where
                // the selection widget is reachable; upstream defines the
                // same lambda inside initChildWidgets.
            }),
        );
        let mut open_button = open_button;
        open_button.visible = visible_more_than_one;
        self.selection_widget = Some(selection_widget);
        self.open_button = Some(open_button);
    }

    pub fn get_x_pos(&self) -> i32 {
        self.output_slot.x + BUTTON_X_OFFSET
    }

    pub fn get_y_pos(&self) -> i32 {
        self.output_slot.y + BUTTON_Y_OFFSET
    }

    /// `AbstractRecipesWidget.selectRecipe` is abstract upstream; here the
    /// subclass routing runs through `initChildWidgets`'s onSelect closure,
    /// so this path only stands for direct calls.
    pub fn select_recipe(&mut self, resource_location: &Identifier) {
        match self.behavior {
            WidgetBehavior::Persistent => {
                self.env
                    .borrow_mut()
                    .send_persistent_recipe_selection_c2s(resource_location);
            }
            WidgetBehavior::Player => {
                let recipe = self.env.borrow().recipe_by_key(resource_location);
                if let Some(recipe) = recipe {
                    self.env.borrow_mut().select_on_player_capability(recipe);
                }
                self.env
                    .borrow_mut()
                    .send_player_recipe_selection_c2s(resource_location);
            }
        }
    }

    pub fn highlight_recipe(&mut self, resource_location: &Identifier) {
        if let Some(widget) = &mut self.selection_widget {
            widget.highlight_button(resource_location);
        }
    }

    /// `setRecipesList`: sort into a TreeSet, refresh the list, show the
    /// open button only above one result, highlight the selected recipe.
    pub fn set_recipes_list(
        &mut self,
        recipes_list: &BTreeSet<RecipePair>,
        selected: Option<&Identifier>,
    ) {
        let sorted: BTreeSet<RecipePair> = recipes_list.iter().cloned().collect();
        if let Some(widget) = &mut self.selection_widget {
            widget.set_recipe_list(&sorted);
        }
        if let Some(button) = &mut self.open_button {
            button.visible = recipes_list.len() > 1;
        }

        if let Some(selected) = selected {
            self.highlight_recipe(selected);
        }
    }

    /// `render`.
    pub fn render(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        if let Some(widget) = &mut self.selection_widget {
            widget.render(gui, mouse_x, mouse_y);
        }
        if let Some(button) = &mut self.open_button {
            if button.visible {
                button.render_widget(gui, mouse_x, mouse_y);
            }
        }
    }

    /// `mouseClicked`: the open button first, then a selection click, then
    /// deactivating an open panel on any other click.
    pub fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        if let Some(button_widget) = &mut self.open_button {
            if button_widget.mouse_clicked(mouse_x, mouse_y, button) {
                if let Some(widget) = &mut self.selection_widget {
                    widget.set_active(!widget.is_active());
                }
                return true;
            }
        }
        if let Some(widget) = &mut self.selection_widget {
            if widget.mouse_clicked(mouse_x, mouse_y, button) {
                widget.set_active(false);
                return true;
            } else if widget.is_active() {
                widget.set_active(false);
                return true;
            }
        }
        false
    }
}

/// `IRecipesWidget` as implemented by the widget layer.
pub trait IRecipesWidget {
    fn init_child_widgets(&mut self);
    fn select_recipe(&mut self, resource_location: &Identifier);
    fn highlight_recipe(&mut self, resource_location: &Identifier);
    fn set_recipes_list(&mut self, recipes_list: &BTreeSet<RecipePair>, selected: Option<&Identifier>);
    fn render(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32);
    fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool;
    fn get_output_slot(&self) -> SlotRef;
    /// `widget instanceof PersistentRecipesWidget`.
    fn is_persistent(&self) -> bool {
        false
    }
}

impl IRecipesWidget for AbstractRecipesWidget {
    fn init_child_widgets(&mut self) {
        AbstractRecipesWidget::init_child_widgets(self)
    }

    fn select_recipe(&mut self, resource_location: &Identifier) {
        AbstractRecipesWidget::select_recipe(self, resource_location)
    }

    fn highlight_recipe(&mut self, resource_location: &Identifier) {
        AbstractRecipesWidget::highlight_recipe(self, resource_location)
    }

    fn set_recipes_list(&mut self, recipes_list: &BTreeSet<RecipePair>, selected: Option<&Identifier>) {
        AbstractRecipesWidget::set_recipes_list(self, recipes_list, selected)
    }

    fn render(&mut self, gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        AbstractRecipesWidget::render(self, gui, mouse_x, mouse_y)
    }

    fn mouse_clicked(&mut self, mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        AbstractRecipesWidget::mouse_clicked(self, mouse_x, mouse_y, button)
    }

    fn get_output_slot(&self) -> SlotRef {
        self.output_slot
    }

    fn is_persistent(&self) -> bool {
        self.behavior == WidgetBehavior::Persistent
    }
}

/// `api/client/base/ITickingRecipesWidget.java`: the tick opt-in no shipped
/// widget uses yet.
pub trait ITickingRecipesWidget {
    fn tick(&mut self);
}

/// `client/RecipesWidget.java`: the static widget holder. The upstream
/// statics become thread-locals; `PolymorphWidgets` and the client env are
/// passed in where upstream reads its singletons.
pub struct RecipesWidgetHolder;

thread_local! {
    static WIDGET: RefCell<Option<Box<dyn IRecipesWidget>>> = const { RefCell::new(None) };
    static LAST_SCREEN: RefCell<Option<u64>> = const { RefCell::new(None) };
    static PENDING_DATA: RefCell<Option<(BTreeSet<RecipePair>, Option<Identifier>)>> =
        const { RefCell::new(None) };
}

impl RecipesWidgetHolder {
    /// `get().isPresent()`.
    pub fn is_present() -> bool {
        WIDGET.with_borrow(|w| w.is_some())
    }

    /// `get()`: run `f` against the live widget when one exists.
    pub fn with_widget<R>(f: impl FnOnce(Option<&mut Box<dyn IRecipesWidget>>) -> R) -> R {
        WIDGET.with_borrow_mut(|w| f(w.as_mut()))
    }

    /// `enqueueRecipesList`.
    pub fn enqueue_recipes_list(recipes_list: BTreeSet<RecipePair>, selected: Option<Identifier>) {
        PENDING_DATA.with_borrow_mut(|p| *p = Some((recipes_list, selected)));
    }

    /// `create`: reuse the widget on the same screen, else resolve through
    /// the registry, init it, and flush any pending recipes list.
    pub fn create(
        container_screen: &Rc<dyn ClientScreen>,
        widgets: &PolymorphWidgetsImpl,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) {
        let same_screen_and_widget = LAST_SCREEN.with_borrow(|s| *s == Some(container_screen.id()))
            && WIDGET.with_borrow(|w| w.is_some());
        if same_screen_and_widget {
            return;
        }
        let mut widget = widgets.get_widget_or_default(container_screen, env);

        if widget.is_some() {
            let widget = widget.as_mut().unwrap();

            if widget.is_persistent() && env.borrow().connection_present() {
                env.borrow_mut().send_block_entity_listener_c2s(true);
            }
            widget.init_child_widgets();
            LAST_SCREEN.with_borrow_mut(|s| *s = Some(container_screen.id()));

            if let Some((list, selected)) = PENDING_DATA.take() {
                widget.set_recipes_list(&list, selected.as_ref());
            }
        } else {
            LAST_SCREEN.with_borrow_mut(|s| *s = None);
        }
        WIDGET.with_borrow_mut(|w| *w = widget);
        PENDING_DATA.with_borrow_mut(|p| *p = None);
    }

    /// `clear`.
    pub fn clear(env: &Rc<RefCell<dyn ClientWidgetEnv>>) {
        let persistent = WIDGET.with_borrow(|w| w.as_ref().map(|w| w.is_persistent()));
        if persistent == Some(true) && env.borrow().connection_present() {
            env.borrow_mut().send_block_entity_listener_c2s(false);
        }
        WIDGET.with_borrow_mut(|w| *w = None);
        LAST_SCREEN.with_borrow_mut(|s| *s = None);
    }
}

/// `PolymorphWidgets.IRecipesWidgetFactory`.
pub trait IRecipesWidgetFactory {
    fn create_widget(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>>;
}

/// `api/client/PolymorphWidgets.java`.
pub trait PolymorphWidgetsApi {
    fn get_current_widget(&self) -> bool;
    fn get_widget_or_default(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>>;
    fn get_widget(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>>;
    fn register_widget(&self, factory: Box<dyn IRecipesWidgetFactory>);
    fn find_result_slot(&self, container_screen: &Rc<dyn ClientScreen>) -> Option<SlotRef>;
}

/// `client/PolymorphWidgetsImpl.java`. The `CopyOnWriteArrayList` becomes a
/// plain list behind a `RefCell`; registration only happens at setup.
pub struct PolymorphWidgetsImpl {
    widget_factories: RefCell<Vec<Box<dyn IRecipesWidgetFactory>>>,
}

impl PolymorphWidgetsImpl {
    pub fn new() -> Self {
        Self { widget_factories: RefCell::new(Vec::new()) }
    }

    /// `setup`: register the built-in menu matcher.
    pub fn setup(&self) {
        self.register_widget(Box::new(BuiltinWidgetFactory));
    }
}

impl Default for PolymorphWidgetsImpl {
    fn default() -> Self {
        Self::new()
    }
}

/// The `setup` factory: smithing binds slot 3 of the player recipes widget,
/// furnaces use the furnace widget, crafters the crafter widget.
struct BuiltinWidgetFactory;

impl IRecipesWidgetFactory for BuiltinWidgetFactory {
    fn create_widget(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>> {
        match container_screen.menu_kind() {
            MenuKind::Smithing => {
                let slot = container_screen
                    .menu_slots()
                    .get(3)
                    .copied()
                    .expect("smithing menu slot 3");
                Some(Box::new(AbstractRecipesWidget::new(
                    container_screen.clone(),
                    env.clone(),
                    WidgetBehavior::Player,
                    slot,
                )))
            }
            MenuKind::AbstractFurnace => Some(Box::new(AbstractRecipesWidget::new_furnace(
                container_screen.clone(),
                env.clone(),
            ))),
            MenuKind::Crafter => Some(Box::new(AbstractRecipesWidget::new_crafter(
                container_screen.clone(),
                env.clone(),
            ))),
            MenuKind::Other => None,
        }
    }
}

impl PolymorphWidgetsApi for PolymorphWidgetsImpl {
    /// `getCurrentWidget`: whether the static holder has a widget.
    fn get_current_widget(&self) -> bool {
        RecipesWidgetHolder::is_present()
    }

    /// `getWidgetOrDefault`: the registered match, else the player widget on
    /// the found result slot.
    fn get_widget_or_default(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>> {
        let widget = self.get_widget(container_screen, env);

        if widget.is_none() {
            let slot = self.find_result_slot(container_screen);

            if let Some(slot) = slot {
                return Some(Box::new(AbstractRecipesWidget::new(
                    container_screen.clone(),
                    env.clone(),
                    WidgetBehavior::Player,
                    slot,
                )));
            }
        }
        widget
    }

    /// `getWidget`: the first factory that returns a widget.
    fn get_widget(
        &self,
        container_screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) -> Option<Box<dyn IRecipesWidget>> {
        for factory in self.widget_factories.borrow().iter() {
            if let Some(widget) = factory.create_widget(container_screen, env) {
                return Some(widget);
            }
        }
        None
    }

    /// `registerWidget`: a null factory is rejected upstream with an error
    /// log; here the caller cannot pass one, so the list just grows.
    fn register_widget(&self, factory: Box<dyn IRecipesWidgetFactory>) {
        self.widget_factories.borrow_mut().push(factory);
    }

    /// `findResultSlot`: the first slot over a `ResultContainer`.
    fn find_result_slot(&self, container_screen: &Rc<dyn ClientScreen>) -> Option<SlotRef> {
        container_screen
            .menu_slots()
            .into_iter()
            .find(|slot| slot.container == ContainerKind::Result)
    }
}

/// `client/PolymorphClientEvents.java`, the four screen event hooks.
pub struct PolymorphClientEvents;

impl PolymorphClientEvents {
    /// `tick`: drop the widget when the player or screen went away.
    pub fn tick(screen_present: bool, env: &Rc<RefCell<dyn ClientWidgetEnv>>) {
        let has_widget = RecipesWidgetHolder::is_present();
        if has_widget {
            if !screen_present {
                RecipesWidgetHolder::clear(env);
            }
            // No shipped widget implements `ITickingRecipesWidget`, so the
            // tick branch stays a no-op exactly as upstream resolves it.
        }
    }

    /// `initScreen`: every screen the widget layer sees is a container
    /// screen in this port, so `create` runs directly.
    pub fn init_screen(
        container_screen: &Rc<dyn ClientScreen>,
        widgets: &PolymorphWidgetsImpl,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
    ) {
        RecipesWidgetHolder::create(container_screen, widgets, env);
    }

    /// `render`.
    pub fn render(gui: &mut dyn GuiGraphics, mouse_x: i32, mouse_y: i32) {
        RecipesWidgetHolder::with_widget(|widget| {
            if let Some(widget) = widget {
                widget.render(gui, mouse_x, mouse_y);
            }
        });
    }

    /// `mouseClick`.
    pub fn mouse_click(mouse_x: f64, mouse_y: f64, button: i32) -> bool {
        RecipesWidgetHolder::with_widget(|widget| match widget {
            Some(widget) => widget.mouse_clicked(mouse_x, mouse_y, button),
            None => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn stack(item: &str) -> ItemStack {
        let mut parts = item.split(':');
        ItemStack {
            item: Identifier::new(parts.next().unwrap(), parts.next().unwrap()),
            count: 1,
            components: BTreeMap::new(),
        }
    }

    fn pair(id: &str, item: Option<&str>) -> RecipePair {
        RecipePair {
            resource_location: Identifier::new("test", id),
            output: match item {
                Some(item) => stack(item),
                None => ItemStack {
                    item: Identifier::new("test", "empty"),
                    count: 0,
                    components: BTreeMap::new(),
                },
            },
        }
    }

    struct TestScreen {
        id: u64,
        slots: Vec<SlotRef>,
        kind: MenuKind,
    }

    impl ClientScreen for TestScreen {
        fn id(&self) -> u64 {
            self.id
        }
        fn menu_slots(&self) -> Vec<SlotRef> {
            self.slots.clone()
        }
        fn screen_left(&self) -> i32 {
            100
        }
        fn screen_top(&self) -> i32 {
            50
        }
        fn menu_kind(&self) -> MenuKind {
            self.kind
        }
    }

    /// A recording env: the sends and the player-capability select land in
    /// one shared log the test holds a second handle to.
    struct TestEnv {
        log: Rc<RefCell<Vec<String>>>,
        connected: bool,
    }

    fn test_env(connected: bool) -> (Rc<RefCell<dyn ClientWidgetEnv>>, Rc<RefCell<Vec<String>>>) {
        let log = Rc::new(RefCell::new(Vec::new()));
        (
            Rc::new(RefCell::new(TestEnv { log: log.clone(), connected })),
            log,
        )
    }

    impl ClientWidgetEnv for TestEnv {
        fn connection_present(&self) -> bool {
            self.connected
        }
        fn recipe_by_key(&self, id: &Identifier) -> Option<crate::recipe_cache::RecipeHolder> {
            Some(crate::recipe_cache::RecipeHolder {
                id: id.clone(),
                recipe: Rc::new(TestRecipe),
            })
        }
        fn select_on_player_capability(&mut self, _recipe: crate::recipe_cache::RecipeHolder) {
            self.log.borrow_mut().push("capability-select".into());
        }
        fn send_block_entity_listener_c2s(&mut self, add: bool) {
            self.log
                .borrow_mut()
                .push(format!("listener:{}", add));
        }
        fn send_persistent_recipe_selection_c2s(&mut self, id: &Identifier) {
            self.log
                .borrow_mut()
                .push(format!("persistent:{}", id.path));
        }
        fn send_player_recipe_selection_c2s(&mut self, id: &Identifier) {
            self.log
                .borrow_mut()
                .push(format!("player:{}", id.path));
        }
    }

    struct TestRecipe;
    impl crate::recipe_cache::Recipe for TestRecipe {
        fn result_item(&self) -> Option<ItemStack> {
            None
        }
        fn is_custom(&self) -> bool {
            false
        }
        fn assemble(&self, _input: &crate::recipe_cache::RecipeInput) -> ItemStack {
            stack("test:result")
        }
    }

    fn slot(index: usize, x: i32, y: i32, container: ContainerKind) -> SlotRef {
        SlotRef { index, x, y, container }
    }

    fn widget(
        screen: &Rc<dyn ClientScreen>,
        env: &Rc<RefCell<dyn ClientWidgetEnv>>,
        behavior: WidgetBehavior,
        output: SlotRef,
    ) -> AbstractRecipesWidget {
        AbstractRecipesWidget::new(screen.clone(), env.clone(), behavior, output)
    }

    #[test]
    fn the_recipe_list_skips_empty_outputs_and_lays_out_the_row() {
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 1,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        let (env, _log) = test_env(false);
        let mut w = widget(&screen, &env, WidgetBehavior::Player, slot(0, 10, 20, ContainerKind::Result));
        w.init_child_widgets();
        let mut list = BTreeSet::new();
        list.insert(pair("a", Some("test:a")));
        list.insert(pair("b", None));
        list.insert(pair("c", Some("test:c")));
        w.set_recipes_list(&list, None);
        let selection = w.selection_widget.as_ref().unwrap();
        assert_eq!(selection.get_output_widgets().len(), 2);
        // Two widgets: xOffset = -25 * floor(1) + 13 = -12.
        assert_eq!(selection.get_output_widgets()[0].get_x(), 100 + 10 - 4 - 12);
        assert_eq!(selection.get_output_widgets()[1].get_x(), 100 + 10 - 4 - 12 + 25);
        assert!(w.open_button.as_ref().unwrap().visible);
    }

    #[test]
    fn a_single_result_hides_the_open_button_and_a_click_selects_the_recipe() {
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 2,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        let (env, log) = test_env(false);
        let mut w = widget(&screen, &env, WidgetBehavior::Player, slot(0, 10, 20, ContainerKind::Result));
        w.init_child_widgets();
        let mut list = BTreeSet::new();
        list.insert(pair("a", Some("test:a")));
        w.set_recipes_list(&list, None);
        assert!(!w.open_button.as_ref().unwrap().visible);

        // Open the panel, then click the output widget: onSelect fires and
        // the panel closes.
        w.selection_widget.as_mut().unwrap().set_active(true);
        let widget_x = w.selection_widget.as_ref().unwrap().get_output_widgets()[0].get_x() as f64 + 1.0;
        let widget_y = w.selection_widget.as_ref().unwrap().get_output_widgets()[0].get_y() as f64 + 1.0;
        assert!(w.mouse_clicked(widget_x, widget_y, 0));
        assert_eq!(
            *log.borrow(),
            vec!["capability-select".to_string(), "player:a".to_string()]
        );
        assert!(!w.selection_widget.as_ref().unwrap().is_active());
    }

    #[test]
    fn the_open_button_toggles_the_panel() {
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 3,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        let (env, _log) = test_env(false);
        let mut w = widget(&screen, &env, WidgetBehavior::Player, slot(0, 10, 20, ContainerKind::Result));
        w.init_child_widgets();
        let mut list = BTreeSet::new();
        list.insert(pair("a", Some("test:a")));
        list.insert(pair("c", Some("test:c")));
        w.set_recipes_list(&list, None);
        let open = w.open_button.as_mut().unwrap();
        open.x = 0;
        open.y = 0;
        // The toggle runs through the widget's click chain.
        assert!(w.mouse_clicked(0.5, 0.5, 0));
        assert!(w.selection_widget.as_ref().unwrap().is_active());
    }

    #[test]
    fn persistent_widgets_send_the_listener_and_persistent_selection_packets() {
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 4,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        let (env, log) = test_env(true);
        let mut w = widget(&screen, &env, WidgetBehavior::Persistent, slot(0, 10, 20, ContainerKind::Result));
        w.init_child_widgets();
        assert!(log.borrow().is_empty());
        w.select_recipe(&Identifier::new("test", "recipe"));
        assert_eq!(*log.borrow(), vec!["persistent:recipe".to_string()]);
    }

    #[test]
    fn the_widgets_impl_matches_menus_and_finds_result_slots() {
        let widgets = PolymorphWidgetsImpl::new();
        widgets.setup();
        let (env, _log) = test_env(false);

        // Furnace: the factory binds slot 2.
        let furnace: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 5,
            slots: vec![
                slot(0, 0, 0, ContainerKind::Other),
                slot(1, 0, 0, ContainerKind::Other),
                slot(2, 8, 8, ContainerKind::Other),
            ],
            kind: MenuKind::AbstractFurnace,
        });
        let widget = widgets.get_widget_or_default(&furnace, &env).unwrap();
        assert_eq!(widget.get_output_slot().index, 2);

        // Crafter: the NonInteractiveResultSlot wins over the first slot.
        let crafter: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 6,
            slots: vec![
                slot(0, 0, 0, ContainerKind::Other),
                slot(1, 4, 4, ContainerKind::NonInteractiveResult),
            ],
            kind: MenuKind::Crafter,
        });
        let widget = widgets.get_widget_or_default(&crafter, &env).unwrap();
        assert_eq!(widget.get_output_slot().index, 1);

        // An unknown menu falls back to the player widget on the
        // ResultContainer slot.
        let smithing: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 7,
            slots: vec![
                slot(0, 0, 0, ContainerKind::Other),
                slot(1, 0, 0, ContainerKind::Other),
                slot(2, 0, 0, ContainerKind::Other),
                slot(3, 0, 0, ContainerKind::Other),
                slot(4, 12, 12, ContainerKind::Result),
            ],
            kind: MenuKind::Other,
        });
        let widget = widgets.get_widget_or_default(&smithing, &env).unwrap();
        assert_eq!(widget.get_output_slot().index, 4);
        assert!(!widget.is_persistent());
    }

    #[test]
    fn create_reuses_the_widget_on_the_same_screen_and_flushes_pending_data() {
        let widgets = PolymorphWidgetsImpl::new();
        widgets.setup();
        let (env, log) = test_env(false);
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 8,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });

        // A recipes list that arrives before the screen enqueues, then
        // lands on widget init.
        let mut list = BTreeSet::new();
        list.insert(pair("a", Some("test:a")));
        list.insert(pair("c", Some("test:c")));
        RecipesWidgetHolder::enqueue_recipes_list(list, Some(Identifier::new("test", "a")));
        RecipesWidgetHolder::create(&screen, &widgets, &env);
        assert!(RecipesWidgetHolder::is_present());
        assert!(log.borrow().is_empty(), "a player widget sends no listener packet");

        // The same screen keeps its widget instead of rebuilding it.
        RecipesWidgetHolder::create(&screen, &widgets, &env);
        assert!(RecipesWidgetHolder::is_present());

        // A screen change clears nothing until clear() runs, but create()
        // resolves a fresh widget; a menu with no result slot yields none.
        let bare: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 9,
            slots: vec![slot(0, 0, 0, ContainerKind::Other)],
            kind: MenuKind::Other,
        });
        RecipesWidgetHolder::create(&bare, &widgets, &env);
        assert!(!RecipesWidgetHolder::is_present());
    }

    #[test]
    fn the_client_events_route_into_the_holder() {
        let widgets = PolymorphWidgetsImpl::new();
        widgets.setup();
        let (env, log) = test_env(false);
        let screen: Rc<dyn ClientScreen> = Rc::new(TestScreen {
            id: 10,
            slots: vec![slot(0, 10, 20, ContainerKind::Result)],
            kind: MenuKind::Other,
        });
        PolymorphClientEvents::init_screen(&screen, &widgets, &env);
        assert!(RecipesWidgetHolder::is_present());
        assert!(!PolymorphClientEvents::mouse_click(-1.0, -1.0, 0));

        // Tick with no screen clears the widget; a player widget sends no
        // listener packet either way.
        PolymorphClientEvents::tick(false, &env);
        assert!(!RecipesWidgetHolder::is_present());
        assert!(log.borrow().is_empty());
    }
}
