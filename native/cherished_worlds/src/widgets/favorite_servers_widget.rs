//! 1:1 port of client/favorites/impl/FavoriteServersWidget.

use crate::favorites_list::FavoritesList;
use crate::seams::{
    Screen, ServerSelectionEntry, ServerSelectionList,
};
use crate::widgets::abstract_favorites_list_widget::AbstractFavoritesListWidget;

// java: public class FavoriteServersWidget extends
//           AbstractFavoritesListWidget<ServerSelectionList,
//           ServerSelectionList.Entry>
pub struct FavoriteServersWidget {
    pub widget: AbstractFavoritesListWidget,
    // java: private final JoinMultiplayerScreen joinMultiplayerScreen;
    pub join_multiplayer_screen: Screen,
    // java: the bound ServerSelectionList this.list
    pub server_list: Option<ServerSelectionList>,
}

impl FavoriteServersWidget {
    // java: public FavoriteServersWidget(JoinMultiplayerScreen
    //           joinMultiplayerScreen) { super(joinMultiplayerScreen);
    //           this.joinMultiplayerScreen = joinMultiplayerScreen; }
    pub fn for_screen(
        screen: Screen,
        list: Option<ServerSelectionList>,
        entry_geometry: Vec<crate::seams::EntryGeometry>,
    ) -> FavoriteServersWidget {
        FavoriteServersWidget {
            widget: AbstractFavoritesListWidget::new(screen.clone(), None, entry_geometry),
            join_multiplayer_screen: screen,
            server_list: list,
        }
    }

    // java: @Override public String getKey(ServerSelectionList.Entry entry)
    pub fn get_key(&self, entry: &ServerSelectionEntry) -> String {
        // java: String key = ""; if (entry instanceof
        //           ServerSelectionList.OnlineServerEntry onlineServerEntry) {
        //           ServerData serverData = onlineServerEntry.getServerData();
        //           key = serverData.name + serverData.ip; }
        //       else if (entry instanceof ServerSelectionList.NetworkServerEntry
        //           networkServerEntry) { LanServer serverData =
        //           ((AccessorNetworkServerEntry) networkServerEntry)
        //           .getServerData(); key = serverData.getAddress(); }
        match entry {
            ServerSelectionEntry::Online(data) => data.favorite_key(),
            ServerSelectionEntry::Network(server) => server.address.clone(),
            ServerSelectionEntry::Other => String::new(),
        }
    }

    // java: @Override protected String getNarratableId(ServerSelectionList.Entry
    //           entry)
    pub fn get_narratable_id(&self, entry: &ServerSelectionEntry) -> String {
        match entry {
            // java: id = serverData.name;
            ServerSelectionEntry::Online(data) => data.name.clone(),
            // java: id = serverData.getAddress();
            ServerSelectionEntry::Network(server) => server.address.clone(),
            ServerSelectionEntry::Other => String::new(),
        }
    }

    // java: @Override protected void updateList()
    pub fn update_list(&mut self) {
        if let Some(list) = &mut self.server_list {
            // java: this.list.updateOnlineServers(this.joinMultiplayerScreen
            //           .getServers());
            crate::mixin_hooks::update_online_servers(
                &mut self.join_multiplayer_screen.host.servers,
                &mut list.online_servers,
            );
            // java: ServerSelectionList.Entry selected = this.list.getSelected();
            let selected = list.get_selected();

            // java: if (selected instanceof OnlineServerEntry || selected
            //           instanceof NetworkServerEntry) {
            if matches!(
                selected,
                Some(ServerSelectionEntry::Online(_)) | Some(ServerSelectionEntry::Network(_))
            ) {
                // java: deleteButton.active = !FavoritesList.contains(
                //           this.getKey(selected));
                self.join_multiplayer_screen.host.delete_button_active =
                    !FavoritesList::contains(&self.get_key(&selected.unwrap()));
            }
        }
    }

}
