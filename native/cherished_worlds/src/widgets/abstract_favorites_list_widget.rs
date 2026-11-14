//! 1:1 port of client/favorites/AbstractFavoritesListWidget.

use crate::favorites_list::FavoritesList;
use crate::seams::{
    Component, ComponentPath, EntryGeometry, FocusDirection, FocusNavigationEvent,
    GuiGraphicsExtractor, IconTexture, KeyEvent, MouseButtonEvent, NarratedElementType,
    NarrationElementOutput, Screen, ScreenRectangle, Tooltip, WidgetTooltipHolder,
    WorldSelectionEntry, WorldSelectionList,
};
use crate::widgets::abstract_favorites_widget::{AbstractFavoritesWidget, TEXTURE_SIZE};
use std::sync::Mutex;

// java: public abstract class AbstractFavoritesListWidget<T extends
// ObjectSelectionList<E>, E extends ObjectSelectionList.Entry<E>> extends
// AbstractFavoritesWidget
pub struct AbstractFavoritesListWidget {
    pub inner: AbstractFavoritesWidget,
    // java: private final WidgetTooltipHolder tooltip = new WidgetTooltipHolder();
    pub tooltip: WidgetTooltipHolder,
    // java: @Nullable protected final T list;
    pub world_list: Option<WorldSelectionList>,
    // java: private E hovered;
    pub hovered: Option<WorldSelectionEntry>,
    // java: private int focusIndex = -1;
    pub focus_index: i32,
    // java: private boolean isFocused;
    pub is_focused: bool,
    // engine-bound geometry of the host list entries
    pub entry_geometry: Vec<EntryGeometry>,
    // java: this.isHovered render-pass state
    pub hovered_now: Mutex<bool>,
    // engine-bound search box value behind FavoriteWorldsWidget.getSearchBox()
    pub search_filter: Option<String>,
}

impl AbstractFavoritesListWidget {
    // java: public AbstractFavoritesListWidget(Screen parentScreen) {
    //           super(parentScreen); this.list = this.getList(); }
    pub fn new(
        parent_screen: Screen,
        list: Option<WorldSelectionList>,
        entry_geometry: Vec<EntryGeometry>,
    ) -> AbstractFavoritesListWidget {
        AbstractFavoritesListWidget {
            inner: AbstractFavoritesWidget::with_screen(parent_screen),
            tooltip: WidgetTooltipHolder::default(),
            world_list: list,
            hovered: None,
            focus_index: -1,
            is_focused: false,
            entry_geometry,
            hovered_now: Mutex::new(false),
            search_filter: None,
        }
    }

    // java: @Nullable protected E getHoveredEntry()
    pub fn get_hovered_entry(&self) -> Option<WorldSelectionEntry> {
        self.hovered.clone()
    }

    // java: @Nullable protected E getFocusedEntry()
    pub fn get_focused_entry(&self) -> Option<WorldSelectionEntry> {
        if self.is_focused && self.focus_index != -1 {
            if let Some(list) = &self.world_list {
                let children = list.children();
                if (self.focus_index as usize) < children.len() {
                    return Some(children[self.focus_index as usize].clone());
                }
            }
        }
        None
    }

    // java: protected abstract String getKey(E entry); — the concrete widgets
    // supply it (levelId, name+ip, or LanServer address).
    pub fn get_key(&self, entry: &WorldSelectionEntry) -> String {
        match &entry.summary {
            Some(summary) => summary.level_id.clone(),
            None => String::new(),
        }
    }

    // java: @Override public boolean shouldTakeFocusAfterInteraction()
    pub fn should_take_focus_after_interaction(&self) -> bool {
        false
    }

    // java: @Override public void setTooltip(@Nullable Tooltip tooltip)
    pub fn set_tooltip(&mut self, tooltip: Option<Tooltip>) {
        self.tooltip.set(tooltip);
    }

    // java: @Override public void setTooltipDelay(@NonNull Duration delay)
    pub fn set_tooltip_delay(&mut self, delay: Option<u64>) {
        self.tooltip.set_delay(delay);
    }

    // java: @Override public boolean isMouseOver(double mouseX, double mouseY)
    //       { return this.isActive() && this.isHovered(); }
    pub fn is_mouse_over(&self) -> bool {
        self.inner.base.is_active() && *self.hovered_now.lock().unwrap()
    }

    // java: @Override protected void extractWidgetRenderState(
    //           GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a)
    pub fn extract_widget_render_state(
        &mut self,
        graphics: &mut GuiGraphicsExtractor,
        mouse_x: i32,
        mouse_y: i32,
    ) {
        if self.world_list.is_none() {
            // java: if (this.list == null) { return; }
            return;
        }
        *self.hovered_now.lock().unwrap() = false;
        let mut has_focus = false;
        self.set_tooltip(None);
        let list = self.world_list.as_ref().unwrap();
        let mut tooltip: Option<Tooltip> = None;
        let mut rectangle = ScreenRectangle::empty();
        let children = list.children();

        for (i, child) in children.iter().enumerate() {
            let key = self.get_key(child);

            // java: if (key.isBlank()) { continue; }
            if key.trim().is_empty() {
                continue;
            }
            let geometry = self
                .entry_geometry
                .get(i)
                .copied()
                .unwrap_or(EntryGeometry {
                    x: 0,
                    y: 0,
                    content_y: 0,
                    content_bottom: 0,
                    content_y_middle: 0,
                    height: 0,
                });
            let min_y = list.get_y();
            let max_y = list.get_bottom();
            let mut top_offset = 0;
            let mut bottom_offset = 0;
            // java: int iconY = child.getContentYMiddle() - 4;
            let mut icon_y = geometry.content_y_middle - 4;

            // java: if (minY > iconY) { topOffset = minY - iconY;
            //           if (topOffset >= TEXTURE_SIZE) { continue; }
            //       } else if ((iconY + TEXTURE_SIZE) > maxY) {
            //           bottomOffset = (iconY + TEXTURE_SIZE) - maxY;
            //           if (bottomOffset >= TEXTURE_SIZE) { continue; } }
            if min_y > icon_y {
                top_offset = min_y - icon_y;
                if top_offset >= TEXTURE_SIZE {
                    continue;
                }
            } else if (icon_y + TEXTURE_SIZE) > max_y {
                bottom_offset = (icon_y + TEXTURE_SIZE) - max_y;
                if bottom_offset >= TEXTURE_SIZE {
                    continue;
                }
            }
            // java: iconY = iconY + topOffset;
            icon_y += top_offset;
            // java: int startV = topOffset; int endV = TEXTURE_SIZE - bottomOffset;
            let start_v = top_offset;
            let end_v = TEXTURE_SIZE - bottom_offset;
            // java: int height = endV - startV;
            let height = end_v - start_v;
            // java: int iconX = child.getX() - 16;
            let icon_x = geometry.x - 16;
            // java: boolean isHovered = graphics.containsPointInScissor(mouseX, mouseY)
            //           && this.areCoordinatesInIcon(iconX, iconY, height, mouseX, mouseY);
            let is_hovered = graphics.contains_point_in_scissor(mouse_x, mouse_y)
                && AbstractFavoritesWidget::are_coordinates_in_icon(
                    icon_x,
                    icon_y,
                    height,
                    mouse_x as f64,
                    mouse_y as f64,
                );
            // java: boolean isFavorite = FavoritesList.contains(key);
            let is_favorite = FavoritesList::contains(&key);

            if is_hovered {
                // java: this.hovered = child; this.isHovered = true;
                self.hovered = Some(child.clone());
                *self.hovered_now.lock().unwrap() = true;
                // java: tooltip = Tooltip.create(Component.translatable(
                //           "selectWorld." + MOD_ID + "." +
                //           (isFavorite ? "unfavorite" : "favorite")), null);
                tooltip = Some(Tooltip::create(
                    Component::translatable(&tooltip_key(is_favorite)),
                    None,
                ));
                // java: rectangle = new ScreenRectangle(iconX, iconY, TEXTURE_SIZE, height);
                rectangle = ScreenRectangle::new(icon_x, icon_y, TEXTURE_SIZE, height);
            }

            // java: icon = isFavorite ? (isHovered ? HOVERED_FAVORITED_ICON :
            //           FAVORITED_ICON) : (isHovered ? HOVERED_EMPTY_ICON : EMPTY_ICON);
            let icon = if is_favorite {
                if is_hovered { IconTexture::HoveredFavorited } else { IconTexture::Favorited }
            } else if is_hovered {
                IconTexture::HoveredEmpty
            } else {
                IconTexture::Empty
            };
            // java: graphics.blit(RenderPipelines.GUI_TEXTURED, icon, iconX, iconY,
            //           0, startV, TEXTURE_SIZE, endV - startV, TEXTURE_SIZE,
            //           TEXTURE_SIZE);
            graphics.blit(icon, icon_x, icon_y, start_v, TEXTURE_SIZE, height);
            // java: boolean isFocused = this.isFocused() && this.focusIndex == i;
            let is_focused_here = self.is_focused && self.focus_index == i as i32;

            if is_focused_here {
                has_focus = true;
                // java: graphics.blit(RenderPipelines.GUI_TEXTURED, FOCUSED_OUTLINE,
                //           iconX, iconY, 0, startV, TEXTURE_SIZE, endV - startV,
                //           TEXTURE_SIZE, TEXTURE_SIZE);
                graphics.blit(IconTexture::FocusedOutline, icon_x, icon_y, start_v, TEXTURE_SIZE, height);

                if !*self.hovered_now.lock().unwrap() {
                    tooltip = Some(Tooltip::create(
                        Component::translatable(&tooltip_key(is_favorite)),
                        None,
                    ));
                    rectangle = ScreenRectangle::new(icon_x, icon_y, TEXTURE_SIZE, height);
                }
            }
        }
        self.set_tooltip(tooltip);
        // java: this.tooltip.refreshTooltipForNextRenderPass(graphics, mouseX,
        //           mouseY, this.isHovered, hasFocus, rectangle);
        self.tooltip.refresh_tooltip_for_next_render_pass(
            graphics,
            mouse_x,
            mouse_y,
            *self.hovered_now.lock().unwrap(),
            has_focus,
            Some(rectangle),
        );
    }

    // java: @Override public void onClick(@NonNull MouseButtonEvent event,
    //           boolean doubleClick) { String key = this.onClick(
    //           this.getHoveredEntry()); this.scrollToLevelId(key); }
    pub fn on_click_event(&mut self, _event: MouseButtonEvent, _double_click: bool) -> String {
        let key = self.on_click(self.get_hovered_entry());
        self.scroll_to_level_id(&key);
        key
    }

    // java: public String onClick(E entry)
    pub fn on_click(&mut self, entry: Option<WorldSelectionEntry>) -> String {
        if let Some(entry) = entry {
            // java: String key = this.getKey(entry);
            let key = self.get_key(&entry);
            // java: boolean isFavorite = FavoritesList.contains(key);
            let is_favorite = FavoritesList::contains(&key);

            if is_favorite {
                FavoritesList::remove(&key);
            } else {
                FavoritesList::add(&key);
            }
            // java: FavoritesList.save();
            FavoritesList::save();
            // java: this.updateList();
            self.update_list();
            return key;
        }
        String::new()
    }

    // java: protected abstract void updateList(); — FavoriteWorldsWidget's body
    // drives the host list refilter through the bound seams.
    pub fn update_list(&mut self) {
        if let Some(list) = &mut self.world_list {
            // java: EditBox textField = this.getSearchBox(); String filter = "";
            //       if (textField != null) { filter = textField.getValue(); }
            let filter = self.search_filter.clone().unwrap_or_default();
            // java: List<LevelSummary> levelSummaries = accessor
            //           .getCurrentlyDisplayedLevels();
            let level_summaries = list.currently_displayed_levels.clone();

            // java: if (levelSummaries != null) { accessor.callFillLevels(filter,
            //           levelSummaries); }
            if level_summaries.is_some() {
                list.fill_levels_filter = Some(filter);
            }
        }
    }

    // java: protected void scrollToLevelId(String key)
    pub fn scroll_to_level_id(&mut self, key: &str) {
        // java: if (!key.isBlank() && this.list != null) {
        if !key.trim().is_empty() {
            if let Some(list) = &self.world_list {
                // java: List<E> children = this.list.children();
                let children = list.children();

                for (i, entry) in children.iter().enumerate() {
                    // java: String id = this.getKey(entry);
                    let id = self.get_key(entry);

                    // java: if (key.equals(id)) { this.setFocusIndex(i); break; }
                    if key == id {
                        self.set_focus_index(i as i32);
                        break;
                    }
                }
            }
        }
    }

    // java: @Override public boolean keyPressed(@NonNull KeyEvent evt)
    pub fn key_pressed(&mut self, evt: KeyEvent) -> bool {
        // java: if (!this.isActive()) { return false; }
        if !self.inner.base.is_active() {
            return false;
        } else if evt.is_selection {
            // java: this.playDownSound(Minecraft.getInstance().getSoundManager());
            crate::seams::play_down_sound();
            // java: String key = this.onClick(this.getFocusedEntry());
            //       this.scrollToLevelId(key); return true;
            let key = self.on_click(self.get_focused_entry());
            self.scroll_to_level_id(&key);
            return true;
        }
        false
    }

    // java: @Override protected void updateWidgetNarration(NarrationElementOutput output)
    pub fn update_widget_narration(&mut self, output: Option<&mut NarrationElementOutput>) {
        let output = match output {
            Some(output) => output,
            None => return,
        };
        // java: E selectedEntry = this.getHoveredEntry();
        let mut selected_entry = self.get_hovered_entry();
        let mut is_focused = false;

        // java: if (selectedEntry == null) { selectedEntry =
        //           this.getFocusedEntry(); if (selectedEntry != null) {
        //           isFocused = true; } }
        if selected_entry.is_none() {
            selected_entry = self.get_focused_entry();
            if selected_entry.is_some() {
                is_focused = true;
            }
        }

        if let Some(selected_entry) = selected_entry {
            // java: String id = this.getKey(selectedEntry);
            let id = self.get_key(&selected_entry);

            // java: if (!id.isBlank()) {
            if !id.trim().is_empty() {
                // java: String key = "gui.narrate.cherishedworlds."; if
                //           (FavoritesList.contains(id)) { key += "unfavorite"; }
                //           else { key += "favorite"; }
                let mut key = String::from("gui.narrate.cherishedworlds.");

                if FavoritesList::contains(&id) {
                    key += "unfavorite";
                } else {
                    key += "favorite";
                }
                // java: output.add(NarratedElementType.TITLE,
                //           Component.translatable(key, this.getNarratableId(
                //           selectedEntry)));
                output.add(
                    NarratedElementType::Title,
                    Component::translatable_with(&key, &[&self.get_narratable_id(&selected_entry)]),
                );

                if is_focused {
                    // java: output.add(NarratedElementType.USAGE,
                    //           Component.translatable("narration.button.usage.focused"));
                    output.add(
                        NarratedElementType::Usage,
                        Component::translatable("narration.button.usage.focused"),
                    );
                } else {
                    // java: output.add(NarratedElementType.USAGE,
                    //           Component.translatable("narration.button.usage.hovered"));
                    output.add(
                        NarratedElementType::Usage,
                        Component::translatable("narration.button.usage.hovered"),
                    );
                }
            }
        }
    }

    // java: protected String getNarratableId(E entry) { return this.getKey(entry); }
    pub fn get_narratable_id(&self, entry: &WorldSelectionEntry) -> String {
        self.get_key(entry)
    }

    // java: @Override public void setFocused(boolean focused)
    pub fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;

        if focused {
            if let Some(list) = &self.world_list {
                // java: if (this.focusIndex == -1) { this.focusIndex = 0; ...
                if self.focus_index == -1 {
                    self.focus_index = 0;
                    let children = list.children();

                    for (i, entry) in children.iter().enumerate() {
                        // java: if (entry == this.list.getSelected()) {
                        //           this.setFocusIndex(i); break; }
                        if Some(entry) == list.get_selected().as_ref() {
                            self.set_focus_index(i as i32);
                            break;
                        }
                    }
                } else {
                    // java: this.applyFocus();
                    self.apply_focus();
                }
            }
        }
    }

    // java: protected void applyFocus()
    pub fn apply_focus(&mut self) {
        if let Some(list) = &mut self.world_list {
            let children = list.children();
            let mut index = self.focus_index as usize;

            // java: if (index >= children.size() || index == -1) { index = 0; }
            if index >= children.len() || self.focus_index == -1 {
                index = 0;
            }
            let _entry = match children.get(index) {
                Some(entry) => entry.clone(),
                None => return,
            };
            let geometry = self
                .entry_geometry
                .get(index)
                .copied()
                .unwrap_or(EntryGeometry {
                    x: 0,
                    y: 0,
                    content_y: 0,
                    content_bottom: 0,
                    content_y_middle: 0,
                    height: 0,
                });
            // java: boolean topClipped = entry.getContentY() < this.list.getY();
            let top_clipped = geometry.content_y < list.get_y();
            // java: boolean bottomClipped = entry.getContentBottom() >
            //           this.list.getBottom();
            let bottom_clipped = geometry.content_bottom > list.get_bottom();

            if top_clipped || bottom_clipped {
                // java: int topDelta = entry.getY() - this.list.getY() - 2;
                let top_delta = geometry.y - list.get_y() - 2;

                if top_delta < 0 {
                    // java: this.list.setScrollAmount(this.list.scrollAmount() + topDelta);
                    let amount = list.scroll_amount() + top_delta as f64;
                    list.set_scroll_amount(amount);
                }
                // java: int bottomDelta = this.list.getBottom() - entry.getY()
                //           - entry.getHeight() - 2;
                let bottom_delta = list.get_bottom() - geometry.y - geometry.height - 2;

                if bottom_delta < 0 {
                    // java: this.list.setScrollAmount(this.list.scrollAmount() - bottomDelta);
                    let amount = list.scroll_amount() - bottom_delta as f64;
                    list.set_scroll_amount(amount);
                }
            }
        }
    }

    // java: protected void setFocusIndex(int focusIndex) { this.focusIndex =
    //           focusIndex; this.applyFocus(); }
    pub fn set_focus_index(&mut self, focus_index: i32) {
        self.focus_index = focus_index;
        self.apply_focus();
    }

    // java: @Override public boolean isFocused()
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    // java: @Override public ComponentPath nextFocusPath(FocusNavigationEvent
    //           navigationEvent)
    pub fn next_focus_path(&mut self, navigation_event: FocusNavigationEvent) -> Option<ComponentPath> {
        let list = match &self.world_list {
            Some(list) => list,
            None => return None,
        };

        // java: if (this.list == null || this.list.children().isEmpty())
        //           { return null; }
        if list.children().is_empty() {
            return None;
        }
        match navigation_event {
            // java: } else if (navigationEvent instanceof
            //           FocusNavigationEvent.ArrowNavigation arrowNavigation) {
            FocusNavigationEvent::Arrow { direction } => {
                // java: int delta = switch (arrowNavigation.direction()) {
                //           case UP -> -1; case DOWN -> 1;
                //           case LEFT, RIGHT -> 0; };
                let delta = match direction {
                    FocusDirection::Up => -1,
                    FocusDirection::Down => 1,
                    FocusDirection::Left | FocusDirection::Right => 0,
                };

                // java: if (delta == 0) { return null; }
                if delta == 0 {
                    return None;
                }

                // java: if (delta + this.focusIndex < 0 || delta + this.focusIndex
                //           >= this.list.children().size()) { return null; }
                if self.focus_index + delta < 0
                    || (self.focus_index + delta) as usize >= list.children().len()
                {
                    return None;
                }
                // java: this.setFocusIndex(this.focusIndex + delta);
                self.set_focus_index(self.focus_index + delta);
                // java: this.parentScreen.triggerImmediateNarration(true);
                self.inner.parent_screen.trigger_immediate_narration(true);
                // java: return ComponentPath.leaf(this);
                Some(ComponentPath::Leaf)
            }
            // java: } else if (navigationEvent instanceof
            //           FocusNavigationEvent.TabNavigation) { this.focusIndex = -1; }
            FocusNavigationEvent::Tab => {
                self.focus_index = -1;
                // java: return super.nextFocusPath(navigationEvent);
                None
            }
        }
    }

    // java: @Override public ScreenRectangle getRectangle()
    pub fn get_rectangle(&self) -> ScreenRectangle {
        if let Some(list) = &self.world_list {
            // java: ScreenRectangle rect = this.list.getRectangle();
            //       return new ScreenRectangle(rect.left() - 16, rect.top(), 16,
            //                                   rect.height());
            let rect = list.get_rectangle();
            return ScreenRectangle::new(rect.left - 16, rect.top, 16, rect.height);
        }
        ScreenRectangle::empty()
    }
}

// java: "selectWorld." + MOD_ID + "." + (isFavorite ? "unfavorite" : "favorite")
pub fn tooltip_key(is_favorite: bool) -> String {
    format!("selectWorld.{}.{}", crate::constants::MOD_ID, if is_favorite { "unfavorite" } else { "favorite" })
}
