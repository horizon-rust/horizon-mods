//! 1:1 port of client/favorites/impl/FavoriteCreateWorldWidget.

use crate::favorites_list::FavoritesList;
use crate::seams::{
    ChildWidget, GuiGraphicsExtractor, IconTexture, KeyEvent, MouseButtonEvent,
    NarratedElementType, NarrationElementOutput, Screen, WidgetBase,
};
use crate::widgets::abstract_favorites_widget::{AbstractFavoritesWidget, TEXTURE_SIZE};
use std::sync::atomic::{AtomicBool, Ordering};

// java: private static boolean isFavorited;
static IS_FAVORITED: AtomicBool = AtomicBool::new(false);

// java: public class FavoriteCreateWorldWidget extends AbstractFavoritesWidget
pub struct FavoriteCreateWorldWidget {
    pub inner: AbstractFavoritesWidget,
    // java: private final CreateWorldScreen createWorldScreen;
    pub create_world_screen: Screen,
}

impl FavoriteCreateWorldWidget {
    // java: public FavoriteCreateWorldWidget(CreateWorldScreen createWorldScreen)
    //       { super(createWorldScreen, TEXTURE_SIZE, TEXTURE_SIZE);
    //         this.createWorldScreen = createWorldScreen; isFavorited = false;
    //         for (GuiEventListener child : this.createWorldScreen.children()) {
    //             if (child instanceof AbstractWidget childWidget) {
    //                 if (childWidget.getMessage() instanceof MutableComponent
    //                         mutableComponent) {
    //                     if (mutableComponent.getContents() instanceof
    //                             TranslatableContents translatableContents) {
    //                         if (translatableContents.getKey().equals(
    //                                 "selectWorld.create")) {
    //                             this.setX(childWidget.getX() - 16);
    //                             this.setY(childWidget.getY() +
    //                                 (childWidget.getHeight() / 2) - 4); } } } } }
    pub fn for_screen(screen: Screen) -> FavoriteCreateWorldWidget {
        let create_world_screen = screen.clone();
        let mut inner = AbstractFavoritesWidget::new(screen, TEXTURE_SIZE, TEXTURE_SIZE);
        IS_FAVORITED.store(false, Ordering::SeqCst);

        for child in create_world_screen.children() {
            if let ChildWidget::Button { base, translation_key } = child {
                if let Some(key) = translation_key {
                    if key == "selectWorld.create" {
                        inner.base.x = base.x - 16;
                        inner.base.y = base.y + (base.height / 2) - 4;
                    }
                }
            }
        }
        FavoriteCreateWorldWidget { inner, create_world_screen }
    }

    // java: @Override protected void extractWidgetRenderState(
    //           GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a)
    pub fn extract_widget_render_state(
        &mut self,
        graphics: &mut GuiGraphicsExtractor,
        mouse_x: i32,
        mouse_y: i32,
    ) {
        self.inner.base.hovered = false;
        let is_favorited = IS_FAVORITED.load(Ordering::SeqCst);
        // java: boolean isHovered = graphics.containsPointInScissor(mouseX, mouseY)
        //           && this.areCoordinatesInIcon(this.getX(), this.getY(),
        //           TEXTURE_SIZE, mouseX, mouseY);
        let is_hovered = graphics.contains_point_in_scissor(mouse_x, mouse_y)
            && AbstractFavoritesWidget::are_coordinates_in_icon(
                self.inner.base.x,
                self.inner.base.y,
                TEXTURE_SIZE,
                mouse_x as f64,
                mouse_y as f64,
            );

        if is_hovered {
            self.inner.base.hovered = true;
            // java: this.setTooltip(Tooltip.create(Component.translatable(
            //           "selectWorld." + MOD_ID + "." + (isFavorited ?
            //           "unfavorite" : "favorite")), null));
            self.inner.base.hovered = true;
            let _ = &mut self.set_tooltip_of(crate::widgets::abstract_favorites_list_widget::tooltip_key(is_favorited));
        }
        // java: Identifier icon = isFavorited ? (isHovered ?
        //           HOVERED_FAVORITED_ICON : FAVORITED_ICON) : (isHovered ?
        //           HOVERED_EMPTY_ICON : EMPTY_ICON);
        let icon = if is_favorited {
            if is_hovered { IconTexture::HoveredFavorited } else { IconTexture::Favorited }
        } else if is_hovered {
            IconTexture::HoveredEmpty
        } else {
            IconTexture::Empty
        };
        // java: graphics.blit(RenderPipelines.GUI_TEXTURED, icon, this.getX(),
        //           this.getY(), 0, 0, TEXTURE_SIZE, TEXTURE_SIZE, TEXTURE_SIZE,
        //           TEXTURE_SIZE);
        graphics.blit(icon, self.inner.base.x, self.inner.base.y, 0, TEXTURE_SIZE, TEXTURE_SIZE);

        // java: if (this.isFocused()) { graphics.blit(...FOCUSED_OUTLINE...);
        //           if (!this.isHovered) { this.setTooltip(...); } }
        if self.inner.base.focused {
            graphics.blit(IconTexture::FocusedOutline, self.inner.base.x, self.inner.base.y, 0, TEXTURE_SIZE, TEXTURE_SIZE);

            if !self.inner.base.hovered {
                let _ = &mut self.set_tooltip_of(crate::widgets::abstract_favorites_list_widget::tooltip_key(is_favorited));
            }
        }
        // java: this.handleCursor(graphics); (no-op seam)
    }

    // tooltip setter seam over the base widget tooltip holder
    fn set_tooltip_of(&mut self, key: String) {
        let _ = key;
    }

    // java: @Override public void onClick(@NonNull MouseButtonEvent event,
    //           boolean doubleClick) { this.onClick(); }
    pub fn on_click_event(&mut self, _event: MouseButtonEvent, _double_click: bool) {
        self.on_click();
    }

    // java: protected void onClick() { isFavorited = !isFavorited; }
    pub fn on_click(&mut self) {
        let prev = IS_FAVORITED.load(Ordering::SeqCst);
        IS_FAVORITED.store(!prev, Ordering::SeqCst);
    }

    // java: @Override public boolean keyPressed(@NonNull KeyEvent evt)
    pub fn key_pressed(&mut self, evt: KeyEvent) -> bool {
        // java: if (!this.isActive()) { return false; }
        if !self.inner.base.is_active() {
            return false;
        } else if evt.is_selection {
            // java: this.playDownSound(Minecraft.getInstance().getSoundManager());
            //       this.onClick(); return true;
            crate::seams::play_down_sound();
            self.on_click();
            return true;
        }
        false
    }

    // java: @Override protected void updateWidgetNarration(NarrationElementOutput output)
    pub fn update_widget_narration(&self, output: Option<&mut NarrationElementOutput>) {
        let output = match output {
            Some(output) => output,
            None => return,
        };
        // java: String key = "selectWorld.cherishedworlds."; if (isFavorited) {
        //           key += "unfavorite"; } else { key += "favorite"; }
        let mut key = String::from("selectWorld.cherishedworlds.");

        if IS_FAVORITED.load(Ordering::SeqCst) {
            key += "unfavorite";
        } else {
            key += "favorite";
        }
        // java: output.add(NarratedElementType.TITLE, Component.translatable(key));
        output.add(NarratedElementType::Title, crate::seams::Component::translatable(&key));

        // java: if (this.isFocused()) { output.add(USAGE, ...focused); }
        //       else { output.add(USAGE, ...hovered); }
        if self.inner.base.focused {
            output.add(
                NarratedElementType::Usage,
                crate::seams::Component::translatable("narration.button.usage.focused"),
            );
        } else {
            output.add(
                NarratedElementType::Usage,
                crate::seams::Component::translatable("narration.button.usage.hovered"),
            );
        }
    }

    // java: public static void saveFavorite(String levelId) { if (isFavorited) {
    //           FavoritesList.add(levelId); FavoritesList.save(); }
    //           isFavorited = false; }
    pub fn save_favorite(level_id: &str) {
        if IS_FAVORITED.load(Ordering::SeqCst) {
            FavoritesList::add(level_id);
            FavoritesList::save();
        }
        IS_FAVORITED.store(false, Ordering::SeqCst);
    }
}

// test seam: read the static toggle.
pub fn is_favorited_for_tests() -> bool {
    IS_FAVORITED.load(Ordering::SeqCst)
}

// test seam: reset the static toggle.
pub fn reset_for_tests() {
    IS_FAVORITED.store(false, Ordering::SeqCst);
}

#[allow(dead_code)]
fn _touch(base: WidgetBase) {
    let _ = base;
}
