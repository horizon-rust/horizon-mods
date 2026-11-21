//! The SMP's view of the server: what the game's own helpers gave the old bundle (`game.native_tell`,
//! `native_open_menu`...), written once over the mod API's `ServerHost`, so the SMP's logic keeps its shape.

use mod_api::{ItemStack, PlayerRef, ServerHost, Text};
use std::path::PathBuf;

/// The server a hook was given, and the player a command acts for.
pub struct Game<'a> {
    pub h: &'a mut dyn ServerHost,
    pub acting: Option<u128>,
}

/// A player on the server, as the SMP reads them.
#[derive(Clone, Debug)]
pub struct NativeServerPlayer {
    pub uuid: u128,
    pub name: String,
    /// The operator level, 0 to 4.
    pub permission: u8,
    /// `minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`.
    pub dimension: String,
    pub position: [f64; 3],
    /// 0 survival, 1 creative, 2 adventure, 3 spectator.
    pub game_mode: i32,
    pub health: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub entity_id: i32,
}

/// Which of a player's inventories (`/invsee`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInventoryKind {
    Main,
    Ender,
}

impl<'a> Game<'a> {
    pub fn new(h: &'a mut dyn ServerHost) -> Self {
        Self { h, acting: None }
    }

    /// The open world's folder.
    pub fn native_world_dir(&self) -> Option<PathBuf> {
        self.h.world_dir()
    }

    pub fn player_ref(&self, uuid: u128) -> Option<PlayerRef> {
        self.h.player(uuid)
    }

    pub fn native_server_players(&self) -> Vec<NativeServerPlayer> {
        let h: &dyn ServerHost = &*self.h;
        h.players()
            .server_players()
            .into_iter()
            .filter_map(|p| read_player(h, p))
            .collect()
    }

    /// A player on the server, as the SMP reads them.
    pub fn player_info(&self, uuid: u128) -> Option<NativeServerPlayer> {
        let h: &dyn ServerHost = &*self.h;
        read_player(h, h.player(uuid)?)
    }

    pub fn native_acting_player(&self) -> Option<NativeServerPlayer> {
        let h: &dyn ServerHost = &*self.h;
        let p = h.players().acting_player()?;
        read_player(h, p)
    }

    pub fn native_tell(&mut self, uuid: u128, line: Text) -> bool {
        let Some(p) = self.h.player(uuid) else {
            return false;
        };
        p.tell(self.h, line).unwrap_or(false)
    }

    pub fn native_action_bar(&mut self, uuid: u128, line: Text) -> bool {
        let Some(p) = self.h.player(uuid) else {
            return false;
        };
        p.action_bar(self.h, line).unwrap_or(false)
    }

    pub fn native_tab_list(&mut self, uuid: u128, header: &Text, footer: &Text) -> bool {
        let Some(p) = self.h.player(uuid) else {
            return false;
        };
        p.tab_list(self.h, header, footer).unwrap_or(false)
    }

    pub fn native_broadcast(&mut self, line: Text) {
        let _ = self.h.messages().broadcast(line);
    }

    /// Feedback to whoever ran the command.
    pub fn native_feedback(&mut self, line: Text) {
        if let Some(uuid) = self.acting {
            self.native_tell(uuid, line);
        } else {
            let _ = self.h.commands().feedback(line);
        }
    }

    pub fn native_run_command(&mut self, line: &str) -> bool {
        self.h.commands().run_command(line)
    }

    pub fn native_text_allowed(&self, text: &str) -> Option<bool> {
        self.h.messages().text_allowed(text)
    }

    pub fn native_tick_stats(&self) -> (f32, f64) {
        self.h.tick_stats()
    }

    pub fn native_set_nickname(&mut self, uuid: u128, nickname: Option<&str>) -> bool {
        let Some(p) = self.h.player(uuid) else {
            return false;
        };
        p.set_nickname(self.h, nickname)
    }

    pub fn native_hide_player(&mut self, uuid: u128, level: Option<u8>) -> bool {
        let Some(p) = self.h.player(uuid) else {
            return false;
        };
        p.hide(self.h, level)
    }

    pub fn native_sleep_ignored(&mut self, uuid: u128, ignored: bool) -> bool {
        let Some(p) = self.h.player(uuid) else {
            return false;
        };
        p.set_sleep_ignored(self.h, ignored)
    }

    pub fn native_send_map(&mut self, uuid: u128, map_id: i32, colors: &[u8]) -> bool {
        let Some(p) = self.h.player(uuid) else {
            return false;
        };
        p.send_map(self.h, map_id, colors)
    }

    /// Opens a chest menu of `rows` rows over `items`: its window id.
    pub fn native_open_menu(
        &mut self,
        uuid: u128,
        rows: u8,
        title: Text,
        items: Vec<Option<ItemStack>>,
    ) -> Option<i32> {
        self.h
            .open_menu(uuid, rows, title, items, mod_api::MenuReplace::CloseFirst)
            .ok()
            .flatten()
    }

    /// Replaces the items of the menu the player has open.
    pub fn native_set_menu_items(&mut self, uuid: u128, items: Vec<Option<ItemStack>>) {
        let Some(p) = self.h.player(uuid) else { return };
        let Some(window) = p
            .container_menu(&*self.h)
            .and_then(|m| m.container_id(&*self.h))
        else {
            return;
        };
        let _ = self.h.menu_set_items(uuid, window, items);
    }

    pub fn native_close_menu(&mut self, uuid: u128) {
        self.h.close_menu(uuid);
    }

    /// The item id of `stack` (`minecraft:stone`).
    pub fn item_id(&self, stack: &ItemStack) -> String {
        stack.item().id(&*self.h)
    }

    pub fn max_stack_size(&self, item: &str) -> Option<u32> {
        let key = self.h.item_key(&mod_api::Identifier::parse(item).ok()?)?;
        self.h.max_stack_size(key)
    }

    /// A stack of `count` `item` (an id), None for an unknown item.
    pub fn stack_of(&self, item: &str, count: u32) -> Option<ItemStack> {
        let key = self.h.item_key(&mod_api::Identifier::parse(item).ok()?)?;
        ItemStack::new(key, count)
    }
}

fn read_player(h: &dyn ServerHost, p: PlayerRef) -> Option<NativeServerPlayer> {
    let entity = p.entity();
    let [yaw, pitch] = entity.rotation(h).unwrap_or([0.0; 2]);
    Some(NativeServerPlayer {
        uuid: p.uuid(),
        name: p.name(h)?,
        permission: p.permission_level(h).unwrap_or(0),
        dimension: p.dimension(h).map(|d| d.to_string()).unwrap_or_default(),
        position: entity.position(h).unwrap_or_default(),
        game_mode: p.game_mode(h).map_or(0, |m| m as i32),
        health: h.living(entity)?.health(h)?,
        yaw,
        pitch,
        entity_id: entity.id() as i32,
    })
}

// ---- items and inventories ----

/// The first slot past the main inventory: 36 to 39 are armour, 40 the off hand.
pub const MAIN: usize = 36;
pub const OFFHAND: usize = 40;

/// Whether `s` has no damage and no component changes: what the market and a trade take as "plain".
pub fn plain(s: &ItemStack) -> bool {
    s.damage_component().is_none() && s.components().is_none_or(|c| c.as_nbt().is_empty())
}

/// The inventory slot (0..9 the hotbar, 9..36 the rest) a click on `slot` of a chest menu of `size` slots
/// landed on, when it is the player's inventory below the menu.
pub fn native_menu_inventory_slot(size: usize, slot: i32) -> Option<usize> {
    let n = usize::try_from(slot).ok()?.checked_sub(size)?;
    match n {
        0..27 => Some(n + 9),
        27..36 => Some(n - 27),
        _ => None,
    }
}

/// Fill matching stacks, then empty slots, in the caller's inventory order.
/// Returns how many of the original stack were inserted.
fn insert(
    slots: &mut [Option<ItemStack>],
    stack: &ItemStack,
    max: u32,
    matching: impl IntoIterator<Item = usize>,
    empty: impl IntoIterator<Item = usize>,
) -> u32 {
    let mut left = stack.count();
    let max = max.max(1);
    for i in matching {
        if let Some(slot) = slots[i].as_mut()
            && slot.is_stackable_with(stack)
            && slot.count() < max
        {
            let n = left.min(max - slot.count());
            slot.set_count(slot.count() + n).expect("positive count");
            left -= n;
        }
        if left == 0 {
            return stack.count();
        }
    }
    for i in empty {
        if slots[i].is_none() {
            let n = left.min(max);
            let mut part = stack.clone();
            part.set_count(n).expect("positive count");
            slots[i] = Some(part);
            left -= n;
        }
        if left == 0 {
            break;
        }
    }
    stack.count() - left
}

fn selected_first(selected: usize) -> impl Iterator<Item = usize> {
    [selected, OFFHAND]
        .into_iter()
        .chain((0..MAIN).filter(move |&i| i != selected))
}

fn write_slots(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    before: &[Option<ItemStack>],
    after: Vec<Option<ItemStack>>,
) -> Result<(), String> {
    for (i, (old, new)) in before.iter().zip(after).enumerate() {
        if *old != new {
            player
                .set_inventory_slot(host, i as u32, new)
                .map_err(|e| format!("{e:?}"))?;
        }
    }
    Ok(())
}

impl<'a> Game<'a> {
    fn pref(&self, uuid: u128) -> Option<PlayerRef> {
        self.h.player(uuid)
    }

    /// A player's inventory slots 0..=40, empty ones None; None for a player who isn't on.
    pub fn native_player_stacks(&self, uuid: u128) -> Option<Vec<Option<ItemStack>>> {
        let p = self.pref(uuid)?;
        let mut out = vec![None; OFFHAND + 1];
        for (slot, stack) in p.inventory_slots(&*self.h)? {
            if let Some(cell) = out.get_mut(slot as usize) {
                *cell = Some(stack);
            }
        }
        Some(out)
    }

    pub fn native_max_stack_size(&self, item: &str) -> Option<u32> {
        self.max_stack_size(item)
    }

    pub fn set_slot(
        &mut self,
        uuid: u128,
        slot: usize,
        stack: Option<ItemStack>,
    ) -> Result<(), String> {
        let p = self.pref(uuid).ok_or("the player is not on the server")?;
        p.set_inventory_slot(self.h, slot as u32, stack)
            .map_err(|e| format!("{e:?}"))
    }

    /// The acting player's main hand: its item, count, and whether it is plain.
    pub fn native_held(&self) -> Option<(String, u32, bool)> {
        let me = self.acting?;
        let p = self.pref(me)?;
        let slot = p.selected_slot(&*self.h)?;
        let stacks = self.native_player_stacks(me)?;
        let s = stacks.get(slot)?.as_ref()?;
        Some((self.item_id(s), s.count(), plain(s)))
    }

    fn acting_stacks(&self) -> Vec<Option<ItemStack>> {
        self.acting
            .and_then(|u| self.native_player_stacks(u))
            .unwrap_or_default()
    }

    /// Every plain item the acting player carries (36 slots and the off hand), by id.
    pub fn native_carried_items(&self) -> Vec<(String, u32)> {
        let stacks = self.acting_stacks();
        let mut out: Vec<(String, u32)> = Vec::new();
        for s in stacks
            .iter()
            .take(MAIN)
            .chain(stacks.get(OFFHAND))
            .flatten()
            .filter(|s| plain(s))
        {
            let id = self.item_id(s);
            match out.iter_mut().find(|(i, _)| *i == id) {
                Some((_, n)) => *n += s.count(),
                None => out.push((id, s.count())),
            }
        }
        out
    }

    fn carried_slots(&self) -> Vec<usize> {
        (0..MAIN).chain([OFFHAND]).collect()
    }

    /// How many of plain `item` the acting player carries.
    pub fn native_count_items(&mut self, item: &str) -> u32 {
        let stacks = self.acting_stacks();
        self.carried_slots()
            .into_iter()
            .filter_map(|i| stacks.get(i)?.as_ref())
            .filter(|s| plain(s) && self.item_id(s) == item)
            .map(|s| s.count())
            .sum()
    }

    /// Takes up to `count` plain `item`, the main hand first; what it took.
    pub fn native_take_items(&mut self, item: &str, count: u32) -> u32 {
        let Some(me) = self.acting else { return 0 };
        let Some(p) = self.pref(me) else { return 0 };
        let selected = p.selected_slot(&*self.h).unwrap_or(0);
        let mut stacks = self.acting_stacks();
        let mut left = count;
        for i in selected_first(selected) {
            if left == 0 {
                break;
            }
            let Some(Some(s)) = stacks.get(i) else {
                continue;
            };
            if !plain(s) || self.item_id(s) != item {
                continue;
            }
            let n = s.count().min(left);
            let rest = (s.count() > n).then(|| {
                let mut r = s.clone();
                let _ = r.set_count(s.count() - n);
                r
            });
            if self.set_slot(me, i, rest.clone()).is_ok() {
                left -= n;
                stacks[i] = rest;
            }
        }
        count - left
    }

    /// How many plain `item` still fit; 0 for an unknown item.
    pub fn native_room_for(&mut self, item: &str) -> u32 {
        let Some(max) = self.max_stack_size(item) else {
            return 0;
        };
        let stacks = self.acting_stacks();
        self.carried_slots()
            .into_iter()
            .map(|i| match stacks.get(i) {
                Some(Some(s)) if plain(s) && self.item_id(s) == item => {
                    max.saturating_sub(s.count())
                }
                Some(None) if i < MAIN => max,
                _ => 0,
            })
            .sum()
    }

    /// Gives up to `count` plain `item` where they fit; what it gave (nothing is dropped).
    pub fn native_give_items(&mut self, item: &str, count: u32) -> u32 {
        let Some(me) = self.acting else { return 0 };
        let Some(stack) = self.stack_of(item, count) else {
            return 0;
        };
        let remaining = self.give_stack_in_order(me, stack, true);
        count - remaining.map_or(0, |s| s.count())
    }

    /// Gives the stack without dropping overflow. A matching offhand stack can fill.
    pub fn native_give_stack(&mut self, uuid: u128, stack: ItemStack) -> Option<ItemStack> {
        self.give_stack_in_order(uuid, stack, false)
    }

    fn give_stack_in_order(
        &mut self,
        uuid: u128,
        mut stack: ItemStack,
        prefer_selected_empty: bool,
    ) -> Option<ItemStack> {
        use mod_api::HostTransaction;

        if self
            .h
            .players()
            .acting_player()
            .is_some_and(|acting| acting.uuid() != uuid)
        {
            return Some(stack);
        }
        let Some(player) = self.pref(uuid) else {
            return Some(stack);
        };
        let Some(selected) = player.selected_slot(&*self.h) else {
            return Some(stack);
        };
        let Some(before) = self.native_player_stacks(uuid) else {
            return Some(stack);
        };
        let mut after = before.clone();
        let max = self.h.max_stack_size(stack.item()).unwrap_or(64);
        let empty = if prefer_selected_empty {
            selected_first(selected)
                .filter(|&i| i < MAIN)
                .collect::<Vec<_>>()
        } else {
            (0..MAIN).collect()
        };
        let added = insert(&mut after, &stack, max, selected_first(selected), empty);
        if added > 0
            && self
                .h
                .transaction(|host, _| write_slots(host, player, &before, after))
                .is_err()
        {
            return Some(stack);
        }
        let remaining = stack.count() - added;
        if remaining == 0 {
            None
        } else {
            stack.set_count(remaining).expect("positive count");
            Some(stack)
        }
    }

    /// Check and plan both offers, then write both inventories in one transaction.
    pub fn native_exchange_items(
        &mut self,
        a: u128,
        a_offer: &[(usize, ItemStack)],
        b: u128,
        b_offer: &[(usize, ItemStack)],
    ) -> Result<(), String> {
        use mod_api::HostTransaction;

        if a == b {
            return Err("a player can't trade with themselves".into());
        }
        let live = |uuid| {
            let player = self.pref(uuid)?;
            (self.h.living(player.entity())?.health(&*self.h)? > 0.0
                && self.h.players().acting_player() != Some(player))
            .then_some((player, self.native_player_stacks(uuid)?))
        };
        let (Some((a_player, a_before)), Some((b_player, b_before))) = (live(a), live(b)) else {
            return Err("both players must be here".into());
        };
        let plan = |before: &[Option<ItemStack>],
                    offered: &[(usize, ItemStack)],
                    received: &[(usize, ItemStack)],
                    who: &str| {
            let mut after = before.to_vec();
            for (slot, expected) in offered {
                let cell = after.get_mut(*slot).filter(|_| *slot < MAIN);
                match cell {
                    Some(cell) if cell.as_ref() == Some(expected) => *cell = None,
                    _ => return Err(format!("{who} no longer holds their offer")),
                }
            }
            for (_, stack) in received {
                let max = self.h.max_stack_size(stack.item()).unwrap_or(64);
                if insert(&mut after, stack, max, 0..MAIN, 0..MAIN) != stack.count() {
                    return Err(format!("{who} has no room"));
                }
            }
            Ok(after)
        };
        let a_after = plan(&a_before, a_offer, b_offer, "the first player")?;
        let b_after = plan(&b_before, b_offer, a_offer, "the second player")?;
        self.h.transaction(|host, _| {
            write_slots(host, a_player, &a_before, a_after)?;
            write_slots(host, b_player, &b_before, b_after)
        })
    }

    pub fn native_inventory(
        &mut self,
        uuid: u128,
        kind: NativeInventoryKind,
    ) -> Result<mod_api::PlayerInventorySnapshot, String> {
        self.h
            .server_player_inventory(uuid, kind.into())
            .map_err(|e| e.to_string())
    }

    pub fn native_move_stack(
        &mut self,
        from: (u128, NativeInventoryKind, usize),
        expected: &ItemStack,
        to: (u128, NativeInventoryKind),
    ) -> Result<(), String> {
        let slot = u32::try_from(from.2).map_err(|_| "that stack changed")?;
        let request = mod_api::PlayerInventoryMove::new(
            (from.0, from.1.into(), slot),
            expected.clone(),
            (to.0, to.1.into()),
        );
        self.h
            .server_move_inventory_stack(&request)
            .map_err(|e| e.to_string())
    }
}

impl From<NativeInventoryKind> for mod_api::PlayerInventoryKind {
    fn from(kind: NativeInventoryKind) -> Self {
        match kind {
            NativeInventoryKind::Main => Self::Player,
            NativeInventoryKind::Ender => Self::Ender,
        }
    }
}

// ---- menu items ----

use mod_api::{Compound, Tag};

/// A text component as NBT (`{text, italic: false, color}`: a name or lore line that isn't in italics).
fn component(text: &str, color: &str) -> Tag {
    let mut c = Compound::new();
    c.insert("text".into(), Tag::String(text.into()));
    c.insert("italic".into(), Tag::Byte(0));
    c.insert("color".into(), Tag::String(color.into()));
    Tag::Compound(c)
}

impl Game<'_> {
    /// `count` of `item` named `name` (not in italics, as a name the game gives); a plain stone for an unknown item.
    pub fn named_stack(&self, item: &str, count: u32, name: &str, color: &str) -> ItemStack {
        let mut stack = self
            .stack_of(item, count.max(1))
            .or_else(|| self.stack_of("minecraft:stone", count.max(1)))
            .expect("the stone item exists");
        let mut patch = Compound::new();
        patch.insert("minecraft:custom_name".into(), component(name, color));
        let _ = stack.apply_components(&patch);
        stack
    }

    pub fn button(&self, item: &str, name: &str, color: &str) -> ItemStack {
        self.named_stack(item, 1, name, color)
    }

    /// Lines under an item's name in a menu.
    pub fn with_lore(&self, mut stack: ItemStack, lines: &[(String, &str)]) -> ItemStack {
        let lines = lines
            .iter()
            .map(|(text, color)| component(text, color))
            .collect();
        let mut patch = Compound::new();
        patch.insert("minecraft:lore".into(), Tag::List(lines));
        let _ = stack.apply_components(&patch);
        stack
    }

    pub fn with_map_id(&self, mut stack: ItemStack, id: i32) -> ItemStack {
        let mut patch = Compound::new();
        patch.insert("minecraft:map_id".into(), Tag::Int(id));
        let _ = stack.apply_components(&patch);
        stack
    }

    /// The map id of a filled map, if it has one.
    pub fn map_id(&self, stack: &ItemStack) -> Option<i32> {
        match stack
            .components()?
            .as_nbt()
            .get(&"minecraft:map_id".into())?
        {
            Tag::Int(id) => Some(*id),
            _ => None,
        }
    }
}

// ---- sidebar, tab list, chunk tickets ----

/// The sidebar the game's helpers once drew: a title and lines of a label and a value.
pub struct NativeSidebar {
    pub title: Text,
    pub lines: Vec<(Text, Option<Text>)>,
}

/// The ticket controller `/rtp`'s chunks are held under, registered at startup.
pub static TICKETS: std::sync::OnceLock<mod_api::TicketController> = std::sync::OnceLock::new();

impl Game<'_> {
    pub fn native_sidebar(&mut self, uuid: u128, sidebar: Option<&NativeSidebar>) -> bool {
        let Some(p) = self.h.player(uuid) else {
            return false;
        };
        let sidebar = sidebar.map(|board| {
            let lines = board
                .lines
                .iter()
                .cloned()
                .map(|(left, right)| mod_api::SidebarLine::new(left, right))
                .collect();
            mod_api::Sidebar::new(board.title.clone(), lines)
        });
        p.send_sidebar(self.h, sidebar).unwrap_or(false)
    }

    /// Holds (or lets go) a chunk loaded for `owner` while `/rtp` looks at it.
    pub fn native_force_chunk_in(
        &mut self,
        _owner: &str,
        dimension: &str,
        chunk: [i32; 2],
        hold: bool,
    ) -> bool {
        let Some(controller) = TICKETS.get().copied() else {
            return false;
        };
        let Some(level) = self.h.level(dimension) else {
            return false;
        };
        let Some(chunk) = level.chunk_ref(&*self.h, chunk) else {
            return false;
        };
        chunk.set_transient_forced(self.h, controller, RTP_OWNER, hold, false)
    }
}

/// The ticket owner of `/rtp`'s chunks.
const RTP_OWNER: u128 = 0x736d_705f_7274_7000_0000_0000_0000_0001;
