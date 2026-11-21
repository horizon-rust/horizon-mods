//! Horizon's official Recipes: every item in a panel beside container screens, with a search box
//! (`@minecraft` keeps a namespace, `#logs` a tag). R over an item, the panel's or any slot's,
//! shows its recipes and U its uses; a click does the same, left for recipes and right for uses.
//! A recipe page has a tab per station, and + beside a crafting recipe the player knows fills the
//! grid as the recipe book does. In creative mode, with "Click gives items" on, a click on a
//! panel item gives one and a right click a stack. Its setting and keys are those of earlier
//! versions (`horizon-mods/.horizon-mod-data/recipes/give`, `key.recipes.recipe` and
//! `key.recipes.uses` in `horizon-mods.txt`), so a player keeps them.
//!
//! The game's items are read once a world is open, its recipes the first time the player uses
//! the panel, and both again when the world's recipes, tags or resources change; nothing is
//! done per frame or per tick.
mod viewer;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use mod_api::{
    ClientHost, ClientPlayerLoggingInEvent, ClientPlayerLoggingOutEvent,
    ClientResourcesReloadedEvent, Hooks, HostSide, Ingredient, ItemKey, ItemStack, KeyRegistration,
    PanelEvent, PanelEventEvent, PanelStyle, RecipeLayout, RecipeResult, RecipeRule, RecipeView,
    RecipesReceivedEvent, Registrar, RegistrationContext, SettingSpec, Stack, TagsUpdateCause,
    TagsUpdatedEvent, Text,
};
use serde_json::Value;

pub use viewer::{
    Event, Item, Layout, MAX_STACKS, Note, Out, Page, Recipe, Station, Tag, Viewer, fill, shaped,
    shapeless_size,
};

/// The mod's id: its settings folder.
pub const ID: &str = "recipes";
/// Its keys: id, label, default (those of earlier versions).
pub const KEYS: [(&str, &str, &str); 2] = [
    ("recipe", "recipes.key.recipe", "key.keyboard.r"),
    ("uses", "recipes.key.uses", "key.keyboard.u"),
];
/// The most items the panel takes.
pub const MAX_ITEMS: usize = 8192;

/// The settings the player picks on Recipes' page in Options > Mods: "Click gives items", kept
/// as earlier versions kept it (`true`/`false` in `give`).
pub fn settings() -> [SettingSpec; 1] {
    [SettingSpec::toggle("give", "recipes.setting.give", false)
        .tooltip("recipes.setting.give.tooltip")]
}

/// The page's words, from the player's language (English when it has none).
#[derive(Clone, Debug, PartialEq)]
pub struct Texts {
    pub recipes: String,
    pub uses: String,
    pub cut: String,
    pub shapeless: String,
    pub cooking: String,
}

impl Texts {
    /// The translation keys and their English.
    pub const KEYS: [(&str, &str); 5] = [
        ("recipes.page.recipes", "Recipes: %s"),
        ("recipes.page.uses", "Uses: %s"),
        ("recipes.page.cut", "%s (%s of %s)"),
        ("recipes.note.shapeless", "Shapeless"),
        ("recipes.note.cooking", "%s s, %s XP"),
    ];

    pub fn new(translate: &dyn Fn(&str) -> Option<String>) -> Self {
        let [recipes, uses, cut, shapeless, cooking] =
            Self::KEYS.map(|(key, english)| translate(key).unwrap_or_else(|| english.into()));
        Texts {
            recipes,
            uses,
            cut,
            shapeless,
            cooking,
        }
    }

    /// A page's title: "Recipes: Stick", with "(256 of 300)" when the tab has more.
    pub fn title(&self, page: &Page, name: &str) -> String {
        let title = fill(if page.uses { &self.uses } else { &self.recipes }, &[name]);
        if page.total > page.recipes.len() {
            let (shown, total) = (page.recipes.len().to_string(), page.total.to_string());
            return fill(&self.cut, &[&title, &shown, &total]);
        }
        title
    }

    /// The line under a recipe.
    pub fn note(&self, note: Note) -> String {
        match note {
            Note::None => String::new(),
            Note::Shapeless => self.shapeless.clone(),
            Note::Cooking { ticks, experience } => {
                let (seconds, xp) = viewer::cooking_numbers(ticks, experience);
                fill(&self.cooking, &[&seconds, &xp])
            }
        }
    }
}

/// How a world's tables name its items: by id and by key, as indexes into the viewer's list.
pub struct Lookup<'a> {
    pub by_id: &'a HashMap<String, u32>,
    pub by_key: &'a HashMap<ItemKey, u32>,
}

impl Lookup<'_> {
    /// An ingredient's items, in the order its slot shows them.
    fn items(&self, ingredient: &Ingredient) -> Vec<u32> {
        (ingredient.order.iter())
            .filter_map(|id| self.by_id.get(id).copied())
            .take(MAX_STACKS)
            .collect()
    }
    /// All of an ingredient's items, as a tag's.
    fn items_all(&self, ingredient: &Ingredient) -> Vec<u32> {
        (ingredient.order.iter())
            .filter_map(|id| self.by_id.get(id).copied())
            .collect()
    }
    fn stack(&self, stack: &ItemStack) -> Option<viewer::Stack> {
        Some(viewer::Stack {
            item: *self.by_key.get(&stack.item())?,
            count: stack.count(),
        })
    }
    fn id(&self, id: &str) -> Option<u32> {
        self.by_id.get(id).copied()
    }
}

/// A recipe of the game as the viewer shows it; none for one it has no page for (a special
/// crafting recipe whose result depends on the grid, a mod's own type) or that makes nothing
/// the game knows.
pub fn convert(r: &mod_api::Recipe, station: Station, items: &Lookup<'_>) -> Option<Recipe> {
    let new = |layout, slots: Vec<Vec<u32>>, result: Vec<viewer::Stack>| {
        (!result.is_empty()).then(|| Recipe {
            id: r.id.clone(),
            station,
            layout,
            width: 1,
            height: 1,
            slots,
            result,
            note: Note::None,
        })
    };
    let one = |item| viewer::Stack { item, count: 1 };
    let optional = |i: &Option<Ingredient>| i.as_ref().map_or_else(Vec::new, |i| items.items(i));
    let mut recipe = match (&r.rule, station) {
        (Some(RecipeRule::Stonecutting { input, result }), Station::Stonecutter) => new(
            Layout::Single,
            vec![items.items(input)],
            items.stack(result).into_iter().collect(),
        )?,
        (
            Some(RecipeRule::SmithingTransform {
                template,
                base,
                addition,
                result,
            }),
            Station::Smithing,
        ) => {
            let base_items = items.items(base);
            let made = match result.item.as_deref() {
                Some(id) => items.id(id).into_iter().collect(),
                None => base_items.clone(),
            };
            let count = result.count.max(1);
            let made = made.into_iter().map(|item| viewer::Stack { item, count });
            let slots = vec![optional(template), base_items, optional(addition)];
            new(Layout::Smithing, slots, made.collect())?
        }
        (
            Some(RecipeRule::SmithingTrim {
                template,
                base,
                addition,
                ..
            }),
            Station::Smithing,
        ) => {
            let base = items.items(base);
            let made = base.iter().map(|&i| one(i)).collect();
            new(
                Layout::Smithing,
                vec![items.items(template), base, items.items(addition)],
                made,
            )?
        }
        // An item and a material that keep the item's contents, as shulker boxes are dyed.
        (
            Some(RecipeRule::Transmute {
                input,
                material,
                result,
                ..
            }),
            Station::Crafting,
        ) => {
            let input = items.items(input);
            let made = match result.item.as_deref() {
                Some(id) => items.id(id).into_iter().collect(),
                None => input.clone(),
            };
            let count = result.count.max(1);
            let made = made.into_iter().map(|item| viewer::Stack { item, count });
            let mut recipe = new(
                Layout::Grid,
                vec![input, items.items(material)],
                made.collect(),
            )?;
            (recipe.width, recipe.height) = shapeless_size(2);
            recipe.note = Note::Shapeless;
            recipe
        }
        (None, Station::Crafting) => {
            let result = r
                .results
                .iter()
                .filter_map(|s| items.stack(s))
                .take(1)
                .collect();
            let cells: Vec<Vec<u32>> = r.ingredients.iter().map(|i| items.items(i)).collect();
            let rows: Option<Vec<&str>> = (r.json.get("pattern").and_then(Value::as_array))
                .map(|rows| rows.iter().filter_map(Value::as_str).collect());
            match rows {
                Some(rows) => {
                    let (width, height, grid) = shaped(&rows, cells)?;
                    let slots = grid.into_iter().map(Option::unwrap_or_default).collect();
                    let mut recipe = new(Layout::Grid, slots, result)?;
                    (recipe.width, recipe.height) = (width, height);
                    recipe
                }
                None => {
                    let (width, height) = shapeless_size(cells.len());
                    let mut slots = cells;
                    slots.resize((width * height) as usize, Vec::new());
                    let mut recipe = new(Layout::Grid, slots, result)?;
                    (recipe.width, recipe.height) = (width, height);
                    recipe.note = Note::Shapeless;
                    recipe
                }
            }
        }
        (None, Station::Furnace | Station::BlastFurnace | Station::Smoker | Station::Campfire) => {
            let input = r.ingredients.first().map(|i| items.items(i))?;
            let result = r
                .results
                .iter()
                .filter_map(|s| items.stack(s))
                .take(1)
                .collect();
            let mut recipe = new(Layout::Cooking, vec![input], result)?;
            recipe.note = Note::Cooking {
                ticks: r.cooking_time.unwrap_or(0),
                experience: r.experience.unwrap_or(0.0),
            };
            recipe
        }
        _ => return None,
    };
    recipe.slots.truncate(27);
    Some(recipe)
}

/// The item tags a recipe's file names (`"#minecraft:logs"`, or `{"tag": "minecraft:logs"}`),
/// as full names: the ones the search knows.
pub fn tags_named(json: &Value, into: &mut BTreeSet<String>) {
    match json {
        Value::String(s) => {
            if let Some(tag) = s.strip_prefix('#').filter(|t| !t.is_empty()) {
                into.insert(qualified(tag));
            }
        }
        Value::Array(list) => list.iter().for_each(|v| tags_named(v, into)),
        Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("tag", Value::String(tag)) => {
                        into.insert(qualified(tag.trim_start_matches('#')));
                    }
                    // A pattern's cells, a result and its components name no tag.
                    ("pattern" | "result" | "components", _) => {}
                    _ => tags_named(value, into),
                }
            }
        }
        _ => {}
    }
}

fn qualified(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

/// A world's tables: the viewer, and how its item indexes map to the game's keys and ids.
struct Tables {
    viewer: Viewer,
    keys: Vec<ItemKey>,
    by_key: HashMap<ItemKey, u32>,
    by_id: HashMap<String, u32>,
    /// Each station's tab icon.
    icons: [Option<ItemKey>; 7],
    /// Whether the recipes were read: not before the player's first use of the panel, since a
    /// world that is still joining has none to give.
    recipes_read: bool,
}

impl Tables {
    /// Every item the game has now, with its name; no recipes yet.
    fn read(host: &dyn ClientHost, search: &str) -> Self {
        let keys = host.item_keys();
        let mut by_key = HashMap::with_capacity(keys.len());
        let mut by_id = HashMap::with_capacity(keys.len());
        let mut items = Vec::with_capacity(keys.len());
        for (n, &key) in keys.iter().enumerate() {
            let id = key.id(host);
            let name = host.hover_name(&ItemStack::of(key));
            by_key.insert(key, n as u32);
            by_id.insert(id.clone(), n as u32);
            items.push(Item { id, name });
        }
        let icons = Station::ALL.map(|s| by_id.get(s.icon()).map(|&n| keys[n as usize]));
        Tables {
            viewer: Viewer::new(items, Vec::new(), Vec::new(), search),
            keys,
            by_key,
            by_id,
            icons,
            recipes_read: false,
        }
    }

    /// Every recipe of the stations' types the game has now, and the tags they name. Read
    /// again next time while the game has none.
    fn read_recipes(&mut self, host: &dyn ClientHost) {
        let lookup = Lookup {
            by_id: &self.by_id,
            by_key: &self.by_key,
        };
        let mut recipes = Vec::new();
        let mut named = BTreeSet::new();
        let mut any = false;
        for kind in Station::types() {
            let Some(station) = Station::of_type(kind) else {
                continue;
            };
            for r in host.recipes_of_type(kind).iter() {
                any = true;
                tags_named(&r.json, &mut named);
                recipes.extend(convert(r, station, &lookup));
            }
        }
        // A tag's items as an ingredient naming it reads them: the `tags()` service isn't
        // served to a client mod's panel events.
        let tags = (named.into_iter())
            .filter_map(|name| {
                let json = serde_json::to_string(&format!("#{name}")).ok()?;
                let ingredient = host.recipe_ingredient(&json).ok()??;
                let items = lookup.items_all(&ingredient);
                (!items.is_empty()).then_some(Tag { name, items })
            })
            .collect();
        self.viewer.set_recipes(recipes, tags);
        self.recipes_read = any;
    }

    fn stack(&self, item: u32, count: u32) -> Stack {
        Stack {
            item: self.keys[item as usize],
            count,
            components: String::new(),
        }
    }

    /// The panel's items now.
    fn listed(&self) -> Vec<Stack> {
        (self.viewer.listed().iter())
            .take(MAX_ITEMS)
            .map(|&n| self.stack(n, 1))
            .collect()
    }

    fn view(&self, n: u32, texts: &Texts) -> RecipeView {
        let r = self.viewer.recipe(n);
        let slots = (r.slots.iter())
            .map(|s| s.iter().map(|&i| self.stack(i, 1)).collect())
            .collect();
        let made = r
            .result
            .iter()
            .map(|s| self.stack(s.item, s.count))
            .collect();
        let layout = match r.layout {
            Layout::Grid => RecipeLayout::Grid,
            Layout::Cooking => RecipeLayout::Cooking,
            Layout::Single => RecipeLayout::Single,
            Layout::Smithing => RecipeLayout::Smithing,
        };
        let mut view = RecipeView::new(layout, slots, vec![RecipeResult::new(made)]);
        (view.width, view.height) = (r.width, r.height);
        view.note = texts.note(r.note);
        view
    }
}

/// A page made ready for the game, its "+" still to be asked.
struct Shown {
    title: String,
    tabs: Vec<Stack>,
    tab: u32,
    views: Vec<RecipeView>,
    /// The id of each recipe whose "+" fills a crafting grid, by its place among the views.
    crafting: Vec<(usize, String)>,
}

#[derive(Default)]
struct State {
    tables: Option<Tables>,
    /// The panel's search text, kept for the next tables.
    search: String,
    texts: Option<Arc<Texts>>,
}

#[derive(Default)]
struct Recipes {
    // Never held across a host call: a host call can fire another of this mod's hooks.
    state: Mutex<State>,
}

impl Recipes {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn texts(&self, host: &dyn ClientHost) -> Arc<Texts> {
        if let Some(texts) = self.state().texts.clone() {
            return texts;
        }
        let texts = Arc::new(Texts::new(&|key| host.translate(key)));
        self.state().texts = Some(texts.clone());
        texts
    }

    /// Reads the world's tables again and shows the items, the search kept; any page goes.
    fn read(&self, host: &mut dyn ClientHost) {
        let search = self.state().search.clone();
        let tables = Tables::read(host, &search);
        let items = tables.listed();
        self.state().tables = Some(tables);
        host.panel_style(ID, PanelStyle::Viewer);
        host.panel_recipes(ID, "", Vec::new(), 0, Vec::new());
        host.panel_items(ID, items);
    }

    /// The recipes, read the first time the player uses the panel. The tables are out of the
    /// state while the game is asked, so that no lock is held across a host call.
    fn read_recipes_once(&self, host: &dyn ClientHost) {
        let tables = {
            let mut state = self.state();
            match &state.tables {
                Some(t) if !t.recipes_read => state.tables.take(),
                _ => None,
            }
        };
        if let Some(mut tables) = tables {
            tables.read_recipes(host);
            let mut state = self.state();
            // Unless the world closed meanwhile.
            if state.tables.is_none() {
                state.tables = Some(tables);
            }
        }
    }

    /// Reads the tables again if a world's were read (a world is open).
    fn read_again(&self, host: &mut dyn ClientHost) {
        if self.state().tables.is_some() {
            self.read(host);
        }
    }

    /// Does what the viewer answered.
    fn apply(&self, host: &mut dyn ClientHost, out: Out) {
        match out {
            Out::Items => {
                let items = self.state().tables.as_ref().map(Tables::listed);
                if let Some(items) = items {
                    host.panel_items(ID, items);
                }
            }
            Out::NoPage => host.panel_recipes(ID, "", Vec::new(), 0, Vec::new()),
            Out::Page(page) => {
                let texts = self.texts(host);
                let Some(shown) = self.shown(&page, &texts) else {
                    return;
                };
                let Shown {
                    title,
                    tabs,
                    tab,
                    mut views,
                    crafting,
                } = shown;
                if !crafting.is_empty() {
                    // Asked now, never kept: the book changes with no hook saying so.
                    let known: HashSet<i32> = host.known_recipes().iter().copied().collect();
                    for (at, id) in crafting {
                        let ids = host.recipe_book_ids(&id);
                        views[at].action = Some(ids.iter().any(|i| known.contains(i)));
                    }
                }
                host.panel_recipes(ID, &title, tabs, tab, views);
            }
            Out::Give { item, count } => {
                let key = self.state().tables.as_ref().map(|t| t.keys[item as usize]);
                if let Some(key) = key {
                    let _ = host.give(key, count);
                }
            }
            Out::Place(n) => {
                let id = (self.state().tables.as_ref()).map(|t| t.viewer.recipe(n).id.clone());
                let Some(id) = id else {
                    return;
                };
                let known = host.known_recipes();
                let ids = host.recipe_book_ids(&id);
                if let Some(&book) = ids.iter().find(|i| known.contains(i))
                    && let Err(e) = host.place_recipe(book, false)
                {
                    let _ = host.chat_line(Text::literal(e));
                }
            }
        }
    }

    fn shown(&self, page: &Page, texts: &Texts) -> Option<Shown> {
        let state = self.state();
        let tables = state.tables.as_ref()?;
        let name = &tables.viewer.items()[page.item as usize].name;
        let air = tables.keys.first().copied()?;
        let tabs = (page.tabs.iter())
            .map(|&s| Stack {
                item: tables.icons[s as usize].unwrap_or(air),
                count: 1,
                components: String::new(),
            })
            .collect();
        let views = page
            .recipes
            .iter()
            .map(|&n| tables.view(n, texts))
            .collect();
        let crafting = (page.recipes.iter().enumerate())
            .filter(|(_, n)| tables.viewer.recipe(**n).station == Station::Crafting)
            .map(|(at, &n)| (at, tables.viewer.recipe(n).id.clone()))
            .collect();
        Some(Shown {
            title: texts.title(page, name),
            tabs,
            tab: page.tab as u32,
            views,
            crafting,
        })
    }

    /// The panel's event as the viewer reads it; none for one it doesn't take.
    fn event(&self, host: &dyn ClientHost, event: &PanelEvent) -> Option<Event> {
        let item = |key: ItemKey| {
            let state = self.state();
            state.tables.as_ref()?.by_key.get(&key).copied()
        };
        Some(match event {
            PanelEvent::Search(text) => {
                self.state().search.clone_from(text);
                Event::Search(text.clone())
            }
            PanelEvent::Clicked { index, right } => Event::Clicked {
                index: *index as usize,
                right: *right,
            },
            PanelEvent::Key { key, stack } => {
                let bound = |id| host.mod_key(ID, id).is_some_and(|(k, _)| k == *key);
                let uses = if bound(KEYS[0].0) {
                    false
                } else if bound(KEYS[1].0) {
                    true
                } else {
                    return None;
                };
                Event::Open {
                    item: item(stack.item)?,
                    uses,
                }
            }
            PanelEvent::SlotClicked { stack, right, .. } => Event::Open {
                item: item(stack.item)?,
                uses: *right,
            },
            PanelEvent::Tab(i) => Event::Tab(*i as usize),
            PanelEvent::Action(i) => Event::Action(*i as usize),
            PanelEvent::Back => Event::Back,
            // The game turns the pages itself.
            _ => return None,
        })
    }

    fn handle(&self, event: Event, give: bool) -> Option<Out> {
        self.state().tables.as_mut()?.viewer.handle(event, give)
    }
}

#[mod_api::horizon_mod]
impl Hooks for Recipes {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for setting in settings() {
            // Only a malformed spec is refused, and the tests check every spec.
            let _ = r.register_setting(setting);
        }
        for (id, label, key) in KEYS {
            r.register_key_mapping(KeyRegistration::new(id, key).label(label));
        }
    }

    /// A world is open: its items and recipes are read, and the panel shows them.
    fn client_player_logging_in(
        &self,
        host: &mut dyn ClientHost,
        _: &mut ClientPlayerLoggingInEvent,
    ) {
        self.read(host);
    }

    /// The world closed: its tables go.
    fn client_player_logging_out(
        &self,
        _: &mut dyn ClientHost,
        _: &mut ClientPlayerLoggingOutEvent,
    ) {
        let mut state = self.state();
        state.tables = None;
        state.search.clear();
    }

    /// The server sent recipes of its own.
    fn recipes_received(&self, host: &mut dyn ClientHost, _: &mut RecipesReceivedEvent<'_>) {
        self.read_again(host);
    }

    /// The server sent new tags (after `/reload`).
    fn tags_updated(&self, host: HostSide, event: &mut TagsUpdatedEvent) {
        if let HostSide::Client(host) = host
            && matches!(event.cause, TagsUpdateCause::ClientPacketReceived { .. })
        {
            self.read_again(host);
        }
    }

    /// A resource reload (a new language among them): the names and words change.
    fn client_resources_reloaded(
        &self,
        host: &mut dyn ClientHost,
        _: &mut ClientResourcesReloadedEvent,
    ) {
        self.state().texts = None;
        self.read_again(host);
    }

    /// The player's search, click or key on this mod's panel.
    fn panel_event(&self, host: &mut dyn ClientHost, event: &mut PanelEventEvent<'_>) {
        if event.mod_id != ID || !host.on(ID) {
            return;
        }
        self.read_recipes_once(host);
        let Some(event) = self.event(host, event.event) else {
            return;
        };
        let give =
            matches!(event, Event::Clicked { .. }) && host.setting_bool("give").unwrap_or(false);
        let Some(out) = self.handle(event.clone(), give) else {
            return;
        };
        if let Out::Give { item, count } = out {
            let key = self.state().tables.as_ref().map(|t| t.keys[item as usize]);
            // Not in creative mode: the item's page, as with the setting off.
            if key.is_some_and(|key| host.give(key, count).is_err())
                && let Some(out) = self.handle(event, false)
            {
                self.apply(host, out);
            }
            return;
        }
        self.apply(host, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_is_one_the_game_takes() {
        for setting in settings() {
            assert_eq!(setting.check(), Ok(()), "{}", setting.key);
        }
    }

    #[test]
    fn the_words_come_from_the_language_else_english() {
        let english = Texts::new(&|_| None);
        assert_eq!(english.recipes, "Recipes: %s");
        let page = Page {
            item: 0,
            uses: true,
            tabs: vec![Station::Crafting],
            tab: 0,
            recipes: vec![0; 256],
            total: 300,
        };
        assert_eq!(english.title(&page, "Stick"), "Uses: Stick (256 of 300)");
        let all = Page {
            uses: false,
            total: 256,
            ..page
        };
        assert_eq!(english.title(&all, "Stick"), "Recipes: Stick");
        let cooking = Note::Cooking {
            ticks: 200,
            experience: 0.15,
        };
        assert_eq!(english.note(cooking), "10 s, 0.15 XP");
        assert_eq!(english.note(Note::Shapeless), "Shapeless");
        assert_eq!(english.note(Note::None), "");
        let other =
            Texts::new(&|key| (key == "recipes.page.recipes").then(|| "Recettes : %s".into()));
        assert_eq!(other.title(&all, "Bâton"), "Recettes : Bâton");
        assert_eq!(other.uses, "Uses: %s");
    }

    #[test]
    fn the_tags_a_recipe_names_are_read_from_its_file() {
        let json: Value = serde_json::from_str(
            r##"{"type": "minecraft:crafting_shaped", "pattern": ["#"],
                "key": {"#": "#logs", "X": ["minecraft:stick", "#minecraft:planks"], "Y": {"tag": "wool"}},
                "result": {"id": "minecraft:stick", "components": {"minecraft:custom_name": "#nope"}}}"##,
        )
        .unwrap();
        let mut named = BTreeSet::new();
        tags_named(&json, &mut named);
        assert_eq!(
            named.into_iter().collect::<Vec<_>>(),
            ["minecraft:logs", "minecraft:planks", "minecraft:wool"]
        );
    }

    fn lookup<R>(ids: &[&str], f: impl FnOnce(&Lookup<'_>) -> R) -> R {
        let by_id = ids
            .iter()
            .enumerate()
            .map(|(n, id)| (id.to_string(), n as u32))
            .collect();
        f(&Lookup {
            by_id: &by_id,
            by_key: &HashMap::new(),
        })
    }

    /// Items the fake game has, by key and id, and a stack of one of them.
    fn fake_items(ids: &[&str]) -> (HashMap<String, u32>, HashMap<ItemKey, u32>, Vec<ItemKey>) {
        let mut host = mod_api::testing::FakeHost::new();
        let names: Vec<_> = ids
            .iter()
            .map(|id| mod_api::Identifier::parse(id).unwrap())
            .collect();
        host.registries.resolve_items(names.clone()).unwrap();
        let keys: Vec<ItemKey> = (names.iter())
            .map(|id| mod_api::prelude::ItemsHost::item_key(&host, id).unwrap())
            .collect();
        let by_id = (ids.iter().enumerate())
            .map(|(n, id)| (id.to_string(), n as u32))
            .collect();
        let by_key = (keys.iter().enumerate())
            .map(|(n, &k)| (k, n as u32))
            .collect();
        (by_id, by_key, keys)
    }

    #[test]
    fn shaped_shapeless_cooking_and_stonecutting_recipes_get_their_layouts() {
        let ids = [
            "minecraft:oak_planks",
            "minecraft:stick",
            "minecraft:stone",
            "minecraft:charcoal",
        ];
        let (by_id, by_key, keys) = fake_items(&ids);
        let items = Lookup {
            by_id: &by_id,
            by_key: &by_key,
        };
        let of = |id: &str| Ingredient::of(vec![id.into()]);
        let stack = |n: usize, count| ItemStack::new(keys[n], count).unwrap();
        let mut sticks = mod_api::Recipe::new(
            "minecraft:stick",
            "minecraft:crafting",
            Arc::new(
                serde_json::json!({"type": "minecraft:crafting_shaped", "pattern": ["# ", " #"]}),
            ),
        );
        sticks.ingredients = vec![of(ids[0]), of(ids[0])];
        sticks.results = vec![stack(1, 4)];
        let r = convert(&sticks, Station::Crafting, &items).unwrap();
        assert_eq!(
            (r.layout, r.width, r.height, r.note),
            (Layout::Grid, 2, 2, Note::None)
        );
        assert_eq!(r.slots, [vec![0], vec![], vec![], vec![0]]);
        assert_eq!(r.result, [viewer::Stack { item: 1, count: 4 }]);
        // Without a pattern, the ingredients fill a small grid.
        sticks.json = Arc::new(serde_json::json!({"type": "minecraft:crafting_shapeless"}));
        let r = convert(&sticks, Station::Crafting, &items).unwrap();
        assert_eq!((r.width, r.height, r.note), (2, 1, Note::Shapeless));
        assert_eq!(r.slots, [vec![0], vec![0]]);
        let mut charcoal = mod_api::Recipe::new(
            "minecraft:charcoal",
            "minecraft:smelting",
            Arc::new(serde_json::json!({})),
        );
        charcoal.ingredients = vec![of(ids[0])];
        charcoal.results = vec![stack(3, 1)];
        (charcoal.cooking_time, charcoal.experience) = (Some(200), Some(0.15));
        let r = convert(&charcoal, Station::Furnace, &items).unwrap();
        assert_eq!(
            (r.layout, r.slots.as_slice()),
            (Layout::Cooking, &[vec![0]][..])
        );
        assert_eq!(
            r.note,
            Note::Cooking {
                ticks: 200,
                experience: 0.15
            }
        );
        let mut cut = mod_api::Recipe::new(
            "minecraft:stone_from_stick",
            "minecraft:stonecutting",
            Arc::new(serde_json::json!({})),
        );
        cut.rule = Some(RecipeRule::Stonecutting {
            input: of(ids[1]),
            result: stack(2, 2),
        });
        let r = convert(&cut, Station::Stonecutter, &items).unwrap();
        assert_eq!(
            (r.layout, r.slots.as_slice()),
            (Layout::Single, &[vec![1]][..])
        );
        assert_eq!(r.result, [viewer::Stack { item: 2, count: 2 }]);
        // A pattern its ingredients don't fill is not shown.
        sticks.json = Arc::new(serde_json::json!({"pattern": ["###"]}));
        assert_eq!(convert(&sticks, Station::Crafting, &items), None);
    }

    #[test]
    fn a_recipe_making_nothing_known_and_a_special_one_get_no_page() {
        let ids = ["minecraft:oak_log", "minecraft:oak_planks"];
        let mut planks = mod_api::Recipe::new(
            "minecraft:oak_planks",
            "minecraft:crafting",
            Arc::new(serde_json::json!({"type": "minecraft:crafting_shapeless"})),
        );
        planks.ingredients = vec![Ingredient::of(vec!["minecraft:oak_log".into()])];
        // No result the game knows.
        lookup(&ids, |items| {
            assert_eq!(convert(&planks, Station::Crafting, items), None)
        });
        let stone = mod_api::Recipe::new(
            "minecraft:stone",
            "minecraft:smelting",
            Arc::new(serde_json::json!({})),
        );
        lookup(&ids, |items| {
            assert_eq!(convert(&stone, Station::Furnace, items), None)
        });
        // A stonecutting recipe without its rule is not one the viewer reads.
        lookup(&ids, |items| {
            assert_eq!(convert(&stone, Station::Stonecutter, items), None)
        });
    }
}
