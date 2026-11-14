//! 1:1 port of client/ScreenEventHooks.

use crate::seams::{Screen, ScreenKind};
use crate::widgets::favorite_create_world_widget::FavoriteCreateWorldWidget;
use crate::widgets::favorite_servers_widget::FavoriteServersWidget;
use crate::widgets::favorite_worlds_widget::FavoriteWorldsWidget;

pub struct ScreenEventHooks;

impl ScreenEventHooks {
    // java: public static void onCreateNewWorld(String levelId)
    pub fn on_create_new_world(level_id: &str) {
        FavoriteCreateWorldWidget::save_favorite(level_id);
    }

    // java: public static void addFavoritesWidget(Screen screen,
    //           Consumer<AbstractWidget> adder)
    // The port answers with which widget the screen gets; the engine takes
    // the returned decision through the hook (seam for Consumer<AbstractWidget>).
    pub fn add_favorites_widget(screen: &Screen) -> WidgetDecision {
        // java: AbstractWidget widget = null;
        let mut widget: WidgetDecision = WidgetDecision::None;

        // java: if (screen instanceof CreateWorldScreen createWorldScreen) { ... }
        match screen.kind() {
            ScreenKind::CreateWorld => {
                widget = WidgetDecision::CreateWorld(FavoriteCreateWorldWidget::for_screen(screen.clone()));
            }
            ScreenKind::JoinMultiplayer => {
                widget = WidgetDecision::JoinMultiplayer(FavoriteServersWidget::for_screen(screen.clone(), screen.host.server_selection_list.clone(), Vec::new()));
            }
            ScreenKind::SelectWorld => {
                widget = WidgetDecision::SelectWorld(FavoriteWorldsWidget::for_screen(screen.clone(), screen.host.world_list.clone(), Vec::new()));
            }
            ScreenKind::Other => {}
        }

        // java: if (widget != null) { adder.accept(widget); }
        widget
    }
}

// java: the constructed AbstractWidget, handed to Consumer<AbstractWidget>.
pub enum WidgetDecision {
    None,
    CreateWorld(FavoriteCreateWorldWidget),
    JoinMultiplayer(FavoriteServersWidget),
    SelectWorld(FavoriteWorldsWidget),
}
