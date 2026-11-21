//! The viewer, with no game in it: a world's items and recipes made into the indexes the search
//! and the recipe pages read, and what the player's clicks, keys and searches on the panel ask
//! the game to do. `lib.rs` fills it from the game and does what it answers. Items and recipes
//! are named by their index in the viewer's lists.

/// What a recipe page holds at most, as the panel takes.
pub const MAX_RECIPES: usize = 256;
/// The stacks a slot cycles through at most, as the panel takes.
pub const MAX_STACKS: usize = 64;
/// A right click's gift in creative mode (the game cuts it to one stack).
pub const STACK: u32 = 99;

/// The stations, in the order of their tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Station {
    Crafting,
    Furnace,
    BlastFurnace,
    Smoker,
    Campfire,
    Stonecutter,
    Smithing,
}

impl Station {
    /// Every station, in the order of the tabs.
    pub const ALL: [Station; 7] = [
        Station::Crafting,
        Station::Furnace,
        Station::BlastFurnace,
        Station::Smoker,
        Station::Campfire,
        Station::Stonecutter,
        Station::Smithing,
    ];

    /// The item its tab shows.
    pub fn icon(self) -> &'static str {
        match self {
            Station::Crafting => "minecraft:crafting_table",
            Station::Furnace => "minecraft:furnace",
            Station::BlastFurnace => "minecraft:blast_furnace",
            Station::Smoker => "minecraft:smoker",
            Station::Campfire => "minecraft:campfire",
            Station::Stonecutter => "minecraft:stonecutter",
            Station::Smithing => "minecraft:smithing_table",
        }
    }

    /// The station of a recipe type (`minecraft:smelting`); none for a type it has no tab for.
    pub fn of_type(kind: &str) -> Option<Self> {
        Some(match kind {
            "minecraft:crafting" => Station::Crafting,
            "minecraft:smelting" => Station::Furnace,
            "minecraft:blasting" => Station::BlastFurnace,
            "minecraft:smoking" => Station::Smoker,
            "minecraft:campfire_cooking" => Station::Campfire,
            "minecraft:stonecutting" => Station::Stonecutter,
            "minecraft:smithing" => Station::Smithing,
            _ => return None,
        })
    }

    /// The recipe types the viewer reads, one per station.
    pub fn types() -> impl Iterator<Item = &'static str> {
        [
            "minecraft:crafting",
            "minecraft:smelting",
            "minecraft:blasting",
            "minecraft:smoking",
            "minecraft:campfire_cooking",
            "minecraft:stonecutting",
            "minecraft:smithing",
        ]
        .into_iter()
    }
}

/// How the panel draws a recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// A crafting grid of the recipe's size.
    Grid,
    /// One input over a flame.
    Cooking,
    /// One input (the stonecutter).
    Single,
    /// Template, base and addition.
    Smithing,
}

/// The line under a recipe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Note {
    None,
    Shapeless,
    /// A cooking recipe's time, in ticks, and the experience it gives.
    Cooking {
        ticks: u32,
        experience: f32,
    },
}

/// An item of the list, with a count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: u32,
    pub count: u32,
}

/// An item the game knows: its id (`minecraft:oak_log`) and the name it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub name: String,
}

/// A recipe, its slots and its result named by their items' indexes.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    /// Its id (`minecraft:stick`), which the recipe book's ids are asked by.
    pub id: String,
    pub station: Station,
    pub layout: Layout,
    /// The grid's size (`Grid`), 1 to 3 each.
    pub width: u32,
    pub height: u32,
    /// Each slot's items, in the order it cycles through them; an empty slot has none.
    pub slots: Vec<Vec<u32>>,
    /// What it makes: one slot cycling through these.
    pub result: Vec<Stack>,
    pub note: Note,
}

/// An item tag the search knows (`#logs`): its name and its items.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    pub name: String,
    pub items: Vec<u32>,
}

/// What the player did on the panel.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The search box's text.
    Search(String),
    /// A click on an item of the panel, by its place in the items shown.
    Clicked {
        index: usize,
        right: bool,
    },
    /// The recipes key (`uses` false) or the uses key over an item, or a click on a recipe's
    /// slot (`uses` for a right click).
    Open {
        item: u32,
        uses: bool,
    },
    Tab(usize),
    /// The "+" of the recipe at this place on the page.
    Action(usize),
    Back,
}

/// A recipe page as the panel shows it: an item's recipes or uses on one station's tab.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub item: u32,
    pub uses: bool,
    /// The stations with recipes for it, in tab order, and the one picked.
    pub tabs: Vec<Station>,
    pub tab: usize,
    /// The recipes shown (at most [`MAX_RECIPES`]), and how many the tab has.
    pub recipes: Vec<u32>,
    pub total: usize,
}

/// What the game is to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// Show [`Viewer::listed`] in the panel.
    Items,
    /// Show this recipe page.
    Page(Page),
    /// No page any more: the items again.
    NoPage,
    /// Creative mode: give the player this item.
    Give { item: u32, count: u32 },
    /// Fill the open grid with this recipe.
    Place(u32),
}

/// An opened page and the tab picked on it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Opened {
    item: u32,
    uses: bool,
    tab: usize,
}

/// The viewer's state while a world is open.
#[derive(Debug, Default)]
pub struct Viewer {
    items: Vec<Item>,
    /// Each item's name in lower case, as the search reads it.
    names: Vec<String>,
    recipes: Vec<Recipe>,
    /// For each item, the recipes that make it and those that take it, in recipe order.
    makes: Vec<Vec<u32>>,
    takes: Vec<Vec<u32>>,
    /// Tags with lower-case names.
    tags: Vec<Tag>,
    /// The search's text in lower case, and the items it keeps.
    search: String,
    listed: Vec<u32>,
    /// The page shown, then the ones Back returns to.
    history: Vec<Opened>,
    /// The recipes the page shows, in order.
    shown: Vec<u32>,
}

impl Viewer {
    /// A world's tables, with the panel's search `search` kept.
    pub fn new(items: Vec<Item>, recipes: Vec<Recipe>, tags: Vec<Tag>, search: &str) -> Self {
        let names = items.iter().map(|i| i.name.to_lowercase()).collect();
        let mut me = Viewer {
            items,
            names,
            search: search.to_lowercase(),
            ..Viewer::default()
        };
        me.set_recipes(recipes, tags);
        me
    }

    /// The world's recipes and the tags the search knows, read again: no page is open any more,
    /// and the search is worked out again.
    pub fn set_recipes(&mut self, recipes: Vec<Recipe>, mut tags: Vec<Tag>) {
        let mut makes = vec![Vec::new(); self.items.len()];
        let mut takes = vec![Vec::new(); self.items.len()];
        for (n, r) in recipes.iter().enumerate() {
            let n = n as u32;
            for s in &r.result {
                push_once(&mut makes, s.item, n);
            }
            for &item in r.slots.iter().flatten() {
                push_once(&mut takes, item, n);
            }
        }
        for tag in &mut tags {
            tag.name.make_ascii_lowercase();
        }
        (self.recipes, self.makes, self.takes, self.tags) = (recipes, makes, takes, tags);
        self.history.clear();
        self.shown.clear();
        self.listed = self.matching(&self.search, None);
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }
    pub fn recipe(&self, n: u32) -> &Recipe {
        &self.recipes[n as usize]
    }
    /// The items the search keeps, in the list's order.
    pub fn listed(&self) -> &[u32] {
        &self.listed
    }

    /// The items `text` (lower case) keeps: every word must be in the name or the id;
    /// `@minecraft` keeps a namespace, `#logs` the items of the tags whose name has it. With
    /// `within`, only those items are looked at.
    fn matching(&self, text: &str, within: Option<&[u32]>) -> Vec<u32> {
        let words: Vec<&str> = text.split_whitespace().collect();
        // Each `#` word's items, worked out once for the whole list.
        let tagged: Vec<Option<Vec<bool>>> = (words.iter())
            .map(|w| {
                let tag = w.strip_prefix('#')?;
                let mut set = vec![false; self.items.len()];
                for t in self.tags.iter().filter(|t| t.name.contains(tag)) {
                    for &i in &t.items {
                        set[i as usize] = true;
                    }
                }
                Some(set)
            })
            .collect();
        let keeps = |n: u32| {
            let item = &self.items[n as usize];
            item.id != "minecraft:air"
                && words.iter().zip(&tagged).all(|(w, tagged)| match tagged {
                    Some(set) => set[n as usize],
                    None => match w.strip_prefix('@') {
                        Some(ns) => item.id.split(':').next().is_some_and(|i| i.starts_with(ns)),
                        None => self.names[n as usize].contains(w) || item.id.contains(w),
                    },
                })
        };
        match within {
            Some(list) => list.iter().copied().filter(|&n| keeps(n)).collect(),
            None => (0..self.items.len() as u32).filter(|&n| keeps(n)).collect(),
        }
    }

    /// The search's new text. A text that goes on from the last one keeps only items the last
    /// one kept, so only those are looked at again.
    fn search(&mut self, text: &str) {
        let text = text.to_lowercase();
        if text == self.search {
            return;
        }
        let within = text
            .starts_with(&self.search)
            .then(|| std::mem::take(&mut self.listed));
        self.listed = self.matching(&text, within.as_deref());
        self.search = text;
    }

    /// The recipes a page lists, every station's.
    fn found(&self, item: u32, uses: bool) -> &[u32] {
        let lists = if uses { &self.takes } else { &self.makes };
        lists.get(item as usize).map_or(&[], Vec::as_slice)
    }

    /// Opens a page on the history; none when the item has no recipes (or uses).
    fn open(&mut self, item: u32, uses: bool) -> Option<Out> {
        if self.found(item, uses).is_empty() {
            return None;
        }
        self.history.push(Opened { item, uses, tab: 0 });
        Some(self.show())
    }

    /// The page on top of the history, or none.
    fn show(&mut self) -> Out {
        let Some(top) = self.history.last_mut() else {
            self.shown.clear();
            return Out::NoPage;
        };
        let (item, uses) = (top.item, top.uses);
        let lists = if uses { &self.takes } else { &self.makes };
        let found = lists.get(item as usize).map_or(&[][..], Vec::as_slice);
        let station = |n: &u32| self.recipes[*n as usize].station;
        let tabs: Vec<Station> = (Station::ALL.into_iter())
            .filter(|&s| found.iter().any(|n| station(n) == s))
            .collect();
        top.tab = top.tab.min(tabs.len().saturating_sub(1));
        let tab = top.tab;
        let on: Vec<u32> = (found.iter())
            .filter(|n| tabs.get(tab) == Some(&station(n)))
            .copied()
            .collect();
        let total = on.len();
        self.shown = on;
        self.shown.truncate(MAX_RECIPES);
        Out::Page(Page {
            item,
            uses,
            tabs,
            tab,
            recipes: self.shown.clone(),
            total,
        })
    }

    /// The player's action; `give` is whether a click on a panel item gives it (the setting,
    /// in creative mode). None when nothing changes.
    pub fn handle(&mut self, event: Event, give: bool) -> Option<Out> {
        match event {
            Event::Search(text) => {
                self.search(&text);
                Some(Out::Items)
            }
            Event::Clicked { index, right } => {
                let item = *self.listed.get(index)?;
                if give {
                    let count = if right { STACK } else { 1 };
                    return Some(Out::Give { item, count });
                }
                self.open(item, right)
            }
            Event::Open { item, uses } => self.open(item, uses),
            Event::Tab(i) => {
                self.history.last_mut()?.tab = i;
                Some(self.show())
            }
            Event::Action(i) => {
                let n = *self.shown.get(i)?;
                (self.recipe(n).station == Station::Crafting).then_some(Out::Place(n))
            }
            Event::Back => {
                self.history.pop()?;
                Some(self.show())
            }
        }
    }
}

fn push_once(lists: &mut [Vec<u32>], item: u32, recipe: u32) {
    if let Some(list) = lists.get_mut(item as usize)
        && list.last() != Some(&recipe)
    {
        list.push(recipe);
    }
}

/// A shaped recipe's grid from its pattern's rows (`["##", "# "]`) and its filled cells' items
/// in reading order, as `Recipe::ingredients` gives them: the width, the height and each cell.
/// None when the pattern isn't 1 to 3 rows of 1 to 3 or doesn't hold one cell per item.
pub fn shaped<T>(rows: &[&str], cells: Vec<T>) -> Option<(u32, u32, Vec<Option<T>>)> {
    let width = rows.iter().map(|r| r.chars().count()).max()?;
    if !(1..=3).contains(&rows.len()) || !(1..=3).contains(&width) {
        return None;
    }
    let mut cells = cells.into_iter();
    let mut grid = Vec::with_capacity(width * rows.len());
    for row in rows {
        let mut n = 0;
        for c in row.chars() {
            grid.push(if c == ' ' { None } else { Some(cells.next()?) });
            n += 1;
        }
        grid.extend((n..width).map(|_| None));
    }
    cells
        .next()
        .is_none()
        .then_some((width as u32, rows.len() as u32, grid))
}

/// A shapeless recipe's grid for `n` ingredients: one row of up to three, else a square.
pub fn shapeless_size(n: usize) -> (u32, u32) {
    let width = match n {
        0 | 1 => 1,
        2..=4 => 2,
        _ => 3,
    };
    (width, (n as u32).div_ceil(width).max(1))
}

/// A translation's text with its arguments: `%s` in order, or `%1$s` by place.
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        if let Some(after) = rest.strip_prefix('s') {
            out.push_str(args.get(next).copied().unwrap_or(""));
            next += 1;
            rest = after;
        } else if let Some(after) = rest.strip_prefix('%') {
            out.push('%');
            rest = after;
        } else if let Some((place, after)) = rest.split_once("$s")
            && let Ok(place) = place.parse::<usize>()
        {
            out.push_str(args.get(place.wrapping_sub(1)).copied().unwrap_or(""));
            rest = after;
        } else {
            out.push('%');
        }
    }
    out.push_str(rest);
    out
}

/// A cooking time in seconds and an experience as the note shows them: `10`, `0.15`.
pub fn cooking_numbers(ticks: u32, experience: f32) -> (String, String) {
    let seconds = ticks as f32 / 20.0;
    (
        (seconds).to_string(),
        ((experience * 100.0).round() / 100.0).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, name: &str) -> Item {
        Item {
            id: id.into(),
            name: name.into(),
        }
    }
    const AIR: u32 = 0;
    const LOG: u32 = 1;
    const PLANKS: u32 = 2;
    const STICK: u32 = 3;
    const CHARCOAL: u32 = 4;
    const OTHER: u32 = 5;

    fn recipe(id: &str, station: Station, slots: Vec<Vec<u32>>, result: Stack) -> Recipe {
        Recipe {
            id: id.into(),
            station,
            layout: Layout::Grid,
            width: 1,
            height: slots.len() as u32,
            slots,
            result: vec![result],
            note: Note::None,
        }
    }

    fn viewer() -> Viewer {
        let items = vec![
            item("minecraft:air", "Air"),
            item("minecraft:oak_log", "Oak Log"),
            item("minecraft:oak_planks", "Oak Planks"),
            item("minecraft:stick", "Stick"),
            item("minecraft:charcoal", "Charcoal"),
            item("other:widget", "Widget"),
        ];
        let planks = Recipe {
            note: Note::Shapeless,
            ..recipe(
                "minecraft:oak_planks",
                Station::Crafting,
                vec![vec![LOG]],
                Stack {
                    item: PLANKS,
                    count: 4,
                },
            )
        };
        let sticks = recipe(
            "minecraft:stick",
            Station::Crafting,
            vec![vec![PLANKS], vec![PLANKS]],
            Stack {
                item: STICK,
                count: 4,
            },
        );
        let charcoal = Recipe {
            layout: Layout::Cooking,
            note: Note::Cooking {
                ticks: 200,
                experience: 0.15,
            },
            ..recipe(
                "minecraft:charcoal",
                Station::Furnace,
                vec![vec![LOG]],
                Stack {
                    item: CHARCOAL,
                    count: 1,
                },
            )
        };
        let tags = vec![Tag {
            name: "minecraft:Logs_That_Burn".into(),
            items: vec![LOG],
        }];
        Viewer::new(items, vec![planks, sticks, charcoal], tags, "")
    }

    fn search(v: &mut Viewer, text: &str) -> Vec<u32> {
        assert_eq!(
            v.handle(Event::Search(text.into()), false),
            Some(Out::Items)
        );
        v.listed().to_vec()
    }

    #[test]
    fn lists_every_item_but_air_and_searches_names_ids_namespaces_and_tags() {
        let mut v = viewer();
        assert_eq!(v.listed(), [LOG, PLANKS, STICK, CHARCOAL, OTHER]);
        assert_eq!(search(&mut v, "OAK"), [LOG, PLANKS]);
        assert_eq!(search(&mut v, "oak_p"), [PLANKS]);
        assert_eq!(search(&mut v, "#logs"), [LOG]);
        assert_eq!(search(&mut v, "#burn oak"), [LOG]);
        assert_eq!(search(&mut v, "@mine stick"), [STICK]);
        assert_eq!(search(&mut v, "@other"), [OTHER]);
        assert_eq!(search(&mut v, "@nope"), Vec::<u32>::new());
        assert_eq!(search(&mut v, ""), [LOG, PLANKS, STICK, CHARCOAL, OTHER]);
    }

    #[test]
    fn a_search_typed_on_keeps_what_the_whole_list_would() {
        let mut v = viewer();
        let typed = [
            "o", "oa", "oak", "oak ", "oak p", "oak pl", "oak", "", "#", "#l", "@", "@o",
        ];
        for text in typed {
            let narrowed = search(&mut v, text);
            assert_eq!(narrowed, v.matching(text, None), "{text:?}");
        }
    }

    #[test]
    fn the_search_survives_new_tables() {
        let v = Viewer::new(
            vec![item("minecraft:stick", "Stick")],
            Vec::new(),
            Vec::new(),
            "ST",
        );
        assert_eq!(v.listed(), [0]);
        let v = Viewer::new(
            vec![item("minecraft:stick", "Stick")],
            Vec::new(),
            Vec::new(),
            "x",
        );
        assert!(v.listed().is_empty());
    }

    #[test]
    fn recipes_read_again_keep_the_search_and_close_the_page() {
        let mut v = viewer();
        assert_eq!(search(&mut v, "#logs"), [LOG]);
        page(v.handle(
            Event::Open {
                item: STICK,
                uses: false,
            },
            false,
        ));
        v.set_recipes(Vec::new(), Vec::new());
        // No tag is known now, so nothing is kept; and no page is open.
        assert!(v.listed().is_empty());
        assert_eq!(v.handle(Event::Back, false), None);
        assert_eq!(
            v.handle(
                Event::Open {
                    item: STICK,
                    uses: false
                },
                false
            ),
            None
        );
    }

    fn page(out: Option<Out>) -> Page {
        match out {
            Some(Out::Page(page)) => page,
            out => panic!("{out:?}"),
        }
    }

    #[test]
    fn a_click_shows_recipes_and_a_right_click_uses_with_a_tab_per_station() {
        let mut v = viewer();
        // The log is listed first: it has no recipe, so nothing opens.
        let click = |right| Event::Clicked { index: 0, right };
        assert_eq!(v.handle(click(false), false), None);
        let uses = page(v.handle(click(true), false));
        assert_eq!((uses.item, uses.uses), (LOG, true));
        assert_eq!(uses.tabs, [Station::Crafting, Station::Furnace]);
        assert_eq!(
            (uses.tab, uses.recipes.as_slice(), uses.total),
            (0, &[0][..], 1)
        );
        let furnace = page(v.handle(Event::Tab(1), false));
        assert_eq!((furnace.tab, furnace.recipes.as_slice()), (1, &[2][..]));
        // A tab past the last is the last.
        assert_eq!(page(v.handle(Event::Tab(9), false)).tab, 1);
        // "+" fills only a crafting grid.
        assert_eq!(v.handle(Event::Action(0), false), None);
        assert_eq!(page(v.handle(Event::Tab(0), false)).tab, 0);
        assert_eq!(v.handle(Event::Action(0), false), Some(Out::Place(0)));
        assert_eq!(v.handle(Event::Action(1), false), None);
        // Back with one page shown: no page, the items again.
        assert_eq!(v.handle(Event::Back, false), Some(Out::NoPage));
        assert_eq!(v.handle(Event::Back, false), None);
        assert_eq!(v.handle(Event::Tab(0), false), None);
    }

    #[test]
    fn keys_and_slot_clicks_open_pages_and_back_returns_through_them() {
        let mut v = viewer();
        let open = |item, uses| Event::Open { item, uses };
        let stick = page(v.handle(open(STICK, false), false));
        assert_eq!((stick.item, stick.recipes.as_slice()), (STICK, &[1][..]));
        assert_eq!(v.handle(open(OTHER, false), false), None);
        let planks = page(v.handle(open(PLANKS, true), false));
        assert_eq!((planks.uses, planks.recipes.as_slice()), (true, &[1][..]));
        page(v.handle(open(LOG, true), false));
        assert_eq!(
            v.handle(Event::Tab(1), false).map(|o| page(Some(o)).tab),
            Some(1)
        );
        let back = page(v.handle(Event::Back, false));
        assert_eq!((back.item, back.uses), (PLANKS, true));
        let back = page(v.handle(Event::Back, false));
        assert_eq!((back.item, back.uses), (STICK, false));
    }

    #[test]
    fn a_recipe_taking_an_item_twice_is_one_use() {
        let v = viewer();
        assert_eq!(v.found(PLANKS, true), [1]);
        assert_eq!(v.found(AIR, true), Vec::<u32>::new().as_slice());
        assert_eq!(v.found(99, false), Vec::<u32>::new().as_slice());
    }

    #[test]
    fn a_tab_shows_its_first_recipes_and_counts_them_all() {
        let items = vec![
            item("minecraft:stone", "Stone"),
            item("minecraft:dirt", "Dirt"),
        ];
        let recipes = (0..MAX_RECIPES + 3)
            .map(|n| {
                recipe(
                    &format!("t:{n}"),
                    Station::Crafting,
                    vec![vec![0]],
                    Stack { item: 1, count: 1 },
                )
            })
            .collect();
        let mut v = Viewer::new(items, recipes, Vec::new(), "");
        let p = page(v.handle(
            Event::Open {
                item: 0,
                uses: true,
            },
            false,
        ));
        assert_eq!((p.recipes.len(), p.total), (MAX_RECIPES, MAX_RECIPES + 3));
        assert_eq!(
            v.handle(Event::Action(MAX_RECIPES - 1), false),
            Some(Out::Place(MAX_RECIPES as u32 - 1))
        );
        assert_eq!(v.handle(Event::Action(MAX_RECIPES), false), None);
    }

    #[test]
    fn with_give_a_click_gives_one_and_a_right_click_a_stack() {
        let mut v = viewer();
        let click = |right| Event::Clicked { index: 2, right };
        assert_eq!(
            v.handle(click(false), true),
            Some(Out::Give {
                item: STICK,
                count: 1
            })
        );
        assert_eq!(
            v.handle(click(true), true),
            Some(Out::Give {
                item: STICK,
                count: STACK
            })
        );
        assert!(matches!(v.handle(click(false), false), Some(Out::Page(_))));
        assert_eq!(
            v.handle(
                Event::Clicked {
                    index: 9,
                    right: false
                },
                true
            ),
            None
        );
    }

    #[test]
    fn a_shaped_pattern_becomes_its_grid() {
        assert_eq!(
            shaped(&["#", "#"], vec!['a', 'b']),
            Some((1, 2, vec![Some('a'), Some('b')]))
        );
        assert_eq!(
            shaped(&["# #", " # "], vec![1, 2, 3]),
            Some((3, 2, vec![Some(1), None, Some(2), None, Some(3), None]))
        );
        // A short row is filled with empty cells.
        assert_eq!(
            shaped(&["##", "#"], vec![1, 2, 3]),
            Some((2, 2, vec![Some(1), Some(2), Some(3), None]))
        );
        // Cells and items that don't agree, and patterns past 3 x 3, are refused.
        assert_eq!(shaped(&["##"], vec![1]), None);
        assert_eq!(shaped(&["#"], vec![1, 2]), None);
        assert_eq!(shaped(&["####"], vec![1, 2, 3, 4]), None);
        assert_eq!(shaped(&["#", "#", "#", "#"], vec![1, 2, 3, 4]), None);
        assert_eq!(shaped::<u8>(&[], vec![]), None);
    }

    #[test]
    fn shapeless_ingredients_fill_a_small_grid() {
        let sizes: Vec<_> = [0, 1, 2, 4, 5, 9].map(shapeless_size).into();
        assert_eq!(sizes, [(1, 1), (1, 1), (2, 1), (2, 2), (3, 2), (3, 3)]);
    }

    #[test]
    fn translations_take_their_arguments_in_order_or_by_place() {
        assert_eq!(fill("Recipes: %s", &["Stick"]), "Recipes: Stick");
        assert_eq!(fill("%2$s de %1$s", &["Stick", "Uses"]), "Uses de Stick");
        assert_eq!(
            fill("%s (%s of %s)", &["Stick", "256", "300"]),
            "Stick (256 of 300)"
        );
        assert_eq!(fill("100%% %s %", &["x"]), "100% x %");
        assert_eq!(fill("%s %s", &["only"]), "only ");
    }

    #[test]
    fn cooking_notes_read_as_before() {
        assert_eq!(cooking_numbers(200, 0.15), ("10".into(), "0.15".into()));
        assert_eq!(cooking_numbers(100, 0.1), ("5".into(), "0.1".into()));
        assert_eq!(cooking_numbers(600, 0.35), ("30".into(), "0.35".into()));
    }
}
