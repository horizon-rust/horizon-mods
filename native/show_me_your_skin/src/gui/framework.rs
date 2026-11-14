//! The owo-braid widget framework and Minecraft client surfaces behind seams.
//! The widget tree itself is an engine/UI seam; each mod widget keeps its Java
//! shape, fields, and state logic exactly, and `build()` describes the tree.

/// Java: io.wispforest.owo.braid.framework.widget.Widget (opaque seam node).
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetNode {
    pub name: String,
    pub children: Vec<WidgetNode>,
}

impl WidgetNode {
    pub fn new(name: &str, children: Vec<WidgetNode>) -> WidgetNode {
        WidgetNode { name: name.to_string(), children }
    }
}

/// Java: net.minecraft.network.chat.Component (text seam).
pub type Component = String;

/// Java: net.minecraft.client.Minecraft client snapshot the screens read:
/// getInstance().player, getGameProfile().id(), and the listed online players
/// (connection.getListedOnlinePlayers() profiles, id + name).
#[derive(Clone, Debug)]
pub struct ClientEnv {
    pub game_profile_id: uuid::Uuid,
    pub player: Option<crate::seams::EntityRef>,
    pub listed_online_players: Vec<(uuid::Uuid, String)>,
    /// Minecraft.getInstance().level != null (the mannequin path checks it).
    pub level_available: bool,
}

thread_local! {
    pub(crate) static CLIENT_ENV: std::cell::RefCell<ClientEnv> = ClientEnv::new().into();
}

impl ClientEnv {
    pub fn new() -> ClientEnv {
        ClientEnv {
            game_profile_id: uuid::Uuid::nil(),
            player: None,
            listed_online_players: Vec::new(),
            level_available: false,
        }
    }

    /// Java: Minecraft.getInstance() reads.
    pub fn current() -> ClientEnv {
        CLIENT_ENV.with(|e| e.borrow().clone())
    }
}

/// Java: Minecraft.getInstance() - the engine glue supplies the live snapshot;
/// tests inject one.
pub fn with_client_env<R>(f: impl FnOnce(&ClientEnv) -> R) -> R {
    CLIENT_ENV.with(|e| f(&e.borrow()))
}

/// Java: mc.gui.setScreen(screen) / BraidScreen.maybeOf(context) parent chain.
pub fn set_screen(_screen: WidgetNode, _parent: Option<WidgetNode>) {
    // Seamed: the Horizon client screen stack binds this at the splice stage.
}

/// Test hook: replace the client snapshot.
#[doc(hidden)]
pub fn set_client_env_for_test(env: ClientEnv) {
    CLIENT_ENV.with(|e| *e.borrow_mut() = env);
}
