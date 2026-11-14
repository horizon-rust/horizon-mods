//! 1:1 port of client/favorites/impl/FavoriteWorldsWidget.

use crate::seams::{Screen, WorldSelectionEntry, WorldSelectionList};
use crate::widgets::abstract_favorites_list_widget::AbstractFavoritesListWidget;

// java: public class FavoriteWorldsWidget extends
//           AbstractFavoritesListWidget<WorldSelectionList,
//           WorldSelectionList.Entry>
pub struct FavoriteWorldsWidget {
    pub widget: AbstractFavoritesListWidget,
}

impl FavoriteWorldsWidget {
    // java: public FavoriteWorldsWidget(SelectWorldScreen selectWorldScreen)
    //       { super(selectWorldScreen); }
    pub fn for_screen(
        screen: Screen,
        list: Option<WorldSelectionList>,
        entry_geometry: Vec<crate::seams::EntryGeometry>,
    ) -> FavoriteWorldsWidget {
        FavoriteWorldsWidget {
            widget: AbstractFavoritesListWidget::new(screen, list, entry_geometry),
        }
    }

    // java: public EditBox getSearchBox() { return ((AccessorWorldSelectionScreen)
    //           this.parentScreen).getSearchBox(); }
    pub fn get_search_box(&self) -> Option<String> {
        self.widget.search_filter.clone()
    }

    // java: @Override public WorldSelectionList getList() { return
    //           ((AccessorWorldSelectionScreen) this.parentScreen).getList(); }
    pub fn get_list(&self) -> Option<&WorldSelectionList> {
        self.widget.world_list.as_ref()
    }

    // java: @Override protected String getKey(WorldSelectionList.Entry entry)
    //       { LevelSummary summary = entry.getLevelSummary();
    //         return summary != null ? summary.getLevelId() : ""; }
    pub fn get_key(&self, entry: &WorldSelectionEntry) -> String {
        self.widget.get_key(entry)
    }
}
