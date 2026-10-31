"""The SMP on a real dedicated server: the built game runs `--dedicated` with the mod, and two real clients (Alex and
Steve, `horizon-ctl --offscreen`) join it. Checked from the clients' own chat and from the files the server wrote:

- the economy opens an account for each player (config.json written with its defaults, then economy.json);
- /balance, /pay (the payer's answer and the payee's line), refused payments (too much, to oneself, unknown name);
- /baltop; /eco is refused below level 3 and works for an operator (made one from the server's console);
- the commands are in each player's command tree (the chat box's suggestions);
- the market (Steve in survival, items given from the console): /worth, /sell <item>, /sell hand, /buy, with the
  items gone from or in his inventory as his client sees it; a creative player can't sell;
- trades (`/trade`, the server's chest menu clicked from each client): closing the menu, a full inventory, the
  cancel button, dying and leaving each end or refuse a trade with nothing moved (each player's items counted from
  their own client); a change of offer clears the other's confirmation; a confirmed trade swaps the items and money;
- coinflips (turned on in config.json before the first start, the other settings filled in): the stake is held from
  the offer, the winner gets both, the limits and the cooldown refuse, and a stake left waiting when the server stops
  comes back; the money in the game stays the same throughout;
- the lottery: two players' tickets held, one drawn after `lottery_seconds`, the winner announced to everyone and paid
  the pot;
- after the server stops cleanly and starts again, the balances are the same, and audit.log holds every change.

Everything runs in fresh temporary folders (the server's, each client's home, launcher and XDG folders), no audio.
"""
import json
import math
import os
import shutil
import socket
import subprocess
import tempfile
import threading
import time
from pathlib import Path

PROPERTIES = {
    "online-mode": "false", "white-list": "false", "enforce-secure-profile": "false", "level-type": "minecraft\\:flat", "spawn-protection": "0", "max-players": "4",
    "view-distance": "4", "simulation-distance": "4", "generate-structures": "false", "enable-command-block": "false",
    "difficulty": "peaceful", "spawn-monsters": "false",
}


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def scratch_env(root):
    env = dict(os.environ, HORIZON_NO_AUDIO="1", HORIZON_LAUNCHER_NO_SELF_UPDATE="1")
    for key in ("HORIZON_HOME", "HORIZON_LAUNCHER_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"):
        path = root / key.lower()
        path.mkdir(parents=True, exist_ok=True)
        env[key] = str(path)
    return env


class Server:
    """`horizon-client --dedicated --nogui` in a scratch server folder, its console on stdin."""

    def __init__(self, game, resources, folder, port):
        self.folder, self.lines, self.game, self.resources, self.port = folder, [], game, resources, port
        folder.mkdir(parents=True, exist_ok=True)
        props = dict(PROPERTIES, **{"server-port": str(port)})
        (folder / "server.properties").write_text("".join(f"{k}={v}\n" for k, v in props.items()))
        (folder / "eula.txt").write_text("eula=true\n")
        self.env = scratch_env(folder.parent / "server-env")

    def start(self):
        self.lines = []
        self.process = subprocess.Popen(
            [str(self.game), "--resources", str(self.resources), "--dedicated", "--nogui"], cwd=self.folder,
            env=self.env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        threading.Thread(target=self._read, daemon=True).start()
        self.wait_for("Done (", 600)

    def _read(self):
        for line in self.process.stdout:
            self.lines.append(line.rstrip("\n"))

    def wait_for(self, text, timeout=120):
        end = time.time() + timeout
        while time.time() < end:
            if any(text in line for line in self.lines):
                return
            if self.process.poll() is not None:
                break
            time.sleep(0.2)
        raise AssertionError(f"server: no {text!r} in {timeout} s; last lines: {self.lines[-30:]}")

    def console(self, line):
        self.process.stdin.write(line + "\n")
        self.process.stdin.flush()

    def stop(self):
        if self.process.poll() is None:
            self.console("stop")
            try:
                self.process.wait(120)
            except subprocess.TimeoutExpired:
                self.process.kill()
        return self.lines


class Client:
    """A real client under horizon-ctl, joined to the server as `name`."""

    def __init__(self, ctl, resources, name, root):
        self.ctl, self.name, self.resources = ctl, name, resources
        self.home = root / name
        self.env = scratch_env(self.home)
        self.socket = str(self.home / "ctl.sock")

    def run(self, *args, check=True):
        done = subprocess.run([str(self.ctl), "--socket", self.socket, *map(str, args)], env=self.env,
                              cwd=self.home, capture_output=True, text=True, timeout=600)
        if check and done.returncode:
            raise AssertionError(f"{self.name}: {args}: {done.stdout[-400:]} {done.stderr[-400:]}")
        try:
            return json.loads(done.stdout.strip().splitlines()[-1])
        except (json.JSONDecodeError, IndexError):
            return done.stdout

    def join(self, port):
        self.run("launch", "--offscreen", "--resources", self.resources)
        self.run("connect", f"127.0.0.1:{port}", self.name)
        self.run("wait-for", "loaded", "--timeout", 300)
        time.sleep(2)

    def say(self, line):
        """A command typed in chat; returns the chat lines it brought."""
        before = len(self.chat())
        self.run("chat", line)
        # The answer, however busy the machine: up to 10 s for a first line,
        # then a moment for the rest of it.
        for _ in range(40):
            time.sleep(0.25)
            if len(self.chat()) > before:
                break
        time.sleep(1)
        return self.chat()[before:]

    def chat(self):
        """Every chat line so far, oldest first (`messages` answers newest first)."""
        reply = self.run("messages", 500)
        return list(reversed(reply.get("chat", [])))

    def quit(self):
        self.run("quit", check=False)

    def screen(self):
        return self.run("screen")

    def menu(self):
        """The open server menu's items by its own slot (horizon-ctl numbers them from 300)."""
        return {e["slot"] - 300: e for e in self.screen().get("slots", []) if e.get("item") and e["slot"] >= 300}

    def slot_of(self, item):
        """The inventory slot (0..36) holding `item`, as the open screen shows it."""
        return next(e["slot"] for e in self.screen().get("slots", []) if e.get("item") == item and e["slot"] < 36)

    def count(self, item):
        counts = self.run("inventory").get("counts", {})
        return counts.get(item, counts.get(item.removeprefix("minecraft:"), 0))

    def click(self, slot):
        self.run("click-slot", slot)
        time.sleep(1)


CONFIRM, CANCEL, THEIR_FIRST = 300 + 45, 300 + 48, 5


def open_trade(alex, steve):
    assert any("Trade request sent to Steve." in l for l in alex.say("/trade Steve"))
    assert any("Alex wants to trade with you" in l for l in steve.chat()[-3:])
    steve.say("/trade accept")
    time.sleep(1)
    for c, other in ((alex, "Steve"), (steve, "Alex")):
        menu = c.menu()
        assert menu.get(45, {}).get("item") == "minecraft:lime_stained_glass_pane", (c.name, menu)
        title = "".join(s["text"] for s in c.screen().get("title", []))
        assert title == f"Trade with {other}", title


def coinflips(alex, steve):
    """Alex 415, Steve 400. Returns what was seen and whether Alex won."""
    results = {}
    results["limits"] = alex.say("/coinflip 5")
    assert any("A coinflip is 10 coins to 1,000 coins." in l for l in results["limits"]), results
    results["offer"] = alex.say("/coinflip 100 Steve")
    assert any("Coinflip of 100 coins offered to Steve" in l for l in results["offer"]), results
    assert any("Balance: 315 coins" in l for l in alex.say("/balance")), "the stake is held"
    assert any("Alex offers you a coinflip of 100 coins" in l for l in steve.chat()[-3:])
    steve.say("/coinflip accept Alex")
    time.sleep(1)
    results["alex_saw"] = alex.chat()[-1:]
    alex_wins = any("you won 200 coins" in l for l in results["alex_saw"])
    assert alex_wins or any("you lost 100 coins" in l for l in results["alex_saw"]), results
    steve_saw = steve.chat()[-2:]
    assert any(("you lost 100 coins" if alex_wins else "you won 200 coins") in l for l in steve_saw), steve_saw
    assert any(f"Balance: {415 + (100 if alex_wins else -100)} coins" in l for l in alex.say("/balance"))
    assert any(f"Balance: {400 + (-100 if alex_wins else 100)} coins" in l for l in steve.say("/balance"))
    results["cooldown"] = steve.say("/coinflip 50")
    assert any("before your next coinflip" in l for l in results["cooldown"]), results
    time.sleep(20)
    assert any("Coinflip of 50 coins offered" in l for l in steve.say("/coinflip 50")), "after the cooldown"
    return results, alex_wins


def lottery(alex, steve, alex_wins):
    """Alex buys 3 tickets, Steve 2; the round draws 30 s after the first. Returns what was seen, whether Alex won."""
    results = {}
    alex_had = 415 + (100 if alex_wins else -100)
    steve_had = 400 + (-100 if alex_wins else 100) - 50  # his coinflip of 50 is still held
    results["buy"] = alex.say("/lottery buy 3")
    assert any("You bought 3 ticket(s) for 30 coins; the pot is 30 coins" in l for l in results["buy"]), results
    assert any("the pot is 50 coins" in l for l in steve.say("/lottery buy 2"))
    assert any(f"Balance: {alex_had - 30} coins" in l for l in alex.say("/balance")), "the tickets are held"
    results["status"] = steve.say("/lottery")
    assert any("Lottery: pot 50 coins, 2 ticket(s) of yours" in l for l in results["status"]), results
    time.sleep(32)
    results["draw"] = [l for l in alex.chat()[-3:] if l.startswith("Lottery:")]
    assert len(results["draw"]) == 1 and "won 50 coins" in results["draw"][0], results
    assert results["draw"][0] in steve.chat()[-3:], "everyone hears the draw"
    alex_lotto = results["draw"][0].startswith("Lottery: Alex won")
    assert any(f"Balance: {alex_had - 30 + (50 if alex_lotto else 0)} coins" in l for l in alex.say("/balance"))
    assert any(f"Balance: {steve_had - 20 + (0 if alex_lotto else 50)} coins" in l for l in steve.say("/balance"))
    return results, alex_lotto


def where(c):
    """The client's dimension, position and health."""
    s = c.run("state")
    return s.get("dimension"), s["player"]["position"], s["player"]["health"]


def near(a, b):
    return sum((x - y) ** 2 for x, y in zip(a, b)) < 1.5 ** 2


def places(server, alex, steve):
    """Spawn, homes, warps, /tpa and /rules; teleports across levels, called off by a move or a hit."""
    r = {}
    r["setspawn"] = alex.say("/setspawn")
    assert any("The spawn is here now." in l for l in r["setspawn"]), r
    r["sethome"] = steve.say("/sethome base")
    assert any('Home "base" set here.' in l for l in r["sethome"]), r
    _, home, _ = where(steve)
    r["limit"] = steve.say("/sethome two")
    assert any("You have 1 of 1 homes" in l for l in r["limit"]), r
    # To the Nether's bedrock roof, then home, across levels.
    server.console("execute in minecraft:the_nether run tp Steve 0.5 128 0.5")
    time.sleep(5)
    assert where(steve)[0] == "minecraft:the_nether", where(steve)
    r["home"] = steve.say("/home base")
    assert any("Teleporting in 2 s: don't move." in l for l in r["home"]), r
    time.sleep(5)
    dim, at, _ = where(steve)
    r["home_reached"] = [dim, at]
    assert dim == "minecraft:overworld" and near(at, home), (r, home)
    assert any('Teleported to home "base".' in l for l in steve.chat()[-4:]), steve.chat()[-4:]
    # A move or a hit during the warmup calls it off.
    steve.say("/spawn")
    steve.run("move", "forward", 1)
    time.sleep(2)
    r["moved"] = steve.chat()[-3:]
    assert any("Teleport called off: you moved." in l for l in r["moved"]), r
    steve.say("/spawn")
    server.console("damage Steve 2")
    time.sleep(3)
    r["hurt"] = steve.chat()[-3:]
    assert any("Teleport called off: you were hurt." in l for l in r["hurt"]), r
    # A request: Alex goes to Steve once Steve accepts.
    alex.say("/tpa Steve")
    assert any("Alex asks to teleport to you" in l for l in steve.chat()[-3:]), steve.chat()[-3:]
    steve.say("/tpaccept")
    time.sleep(4)
    r["tpa"] = [where(alex)[1], where(steve)[1]]
    assert near(*r["tpa"]), r
    alex.say("/setwarp shop")
    r["warps"] = steve.say("/warps")
    assert any("Warps: shop" in l for l in r["warps"]), r
    r["warp_refused"] = steve.say("/setwarp mine")
    assert not any("set here" in l for l in r["warp_refused"]), r
    r["rules"] = steve.say("/rules")
    assert any("1. Be respectful to everyone." in l for l in r["rules"]), r
    return r


def claims(server, alex, steve):
    """Alex claims a chunk: Steve can't break, use or place there until trusted."""
    r = {}
    _, (ax, ay, az), _ = where(alex)
    cx, cz, y = math.floor(ax) >> 4, math.floor(az) >> 4, math.floor(ay)
    r["claim"] = alex.say("/claim")
    assert any("Claimed this chunk" in l for l in r["claim"]), r
    lever, dirt, free = (cx * 16 + 8, y, cz * 16 + 8), (cx * 16 + 10, y, cz * 16 + 8), (cx * 16 + 8, y, cz * 16 + 10)
    server.console("setblock %d %d %d minecraft:lever[face=floor]" % lever)
    server.console("setblock %d %d %d minecraft:dirt" % dirt)
    server.console(f"tp Steve {lever[0] + 0.5} {y} {lever[2] + 3.5}")
    server.console("give Steve minecraft:oak_planks 4")
    time.sleep(3)
    block = lambda pos: json.dumps(steve.run("block", *pos))
    steve.run("interact", *lever)
    time.sleep(0.5)
    # The action bar fades after 3 s: read right after the click (a busy machine may take that long to break).
    said = json.dumps(steve.run("messages", 5))
    steve.run("break", *dirt, "--timeout", 4, check=False)
    steve.run("place", *free, "--item", "minecraft:oak_planks", check=False)
    time.sleep(1.5)
    r["refused"] = [block(lever), block(dirt), block(free), said + json.dumps(steve.run("messages", 5))]
    # Not broken (grass may spread onto the dirt meanwhile).
    assert "powered=true" not in r["refused"][0] and "minecraft:air" not in r["refused"][1], r
    assert "oak_planks" not in r["refused"][2], r
    assert "This land is claimed by Alex." in r["refused"][3], r
    r["trust"] = alex.say("/trust Steve")
    assert any("Steve may now build" in l for l in r["trust"]), r
    steve.run("interact", *lever)
    steve.run("break", *dirt, "--timeout", 10, check=False)
    time.sleep(1.5)
    r["trusted"] = [block(lever), block(dirt)]
    assert "powered=true" in r["trusted"][0] and '"block": "minecraft:air"' in r["trusted"][1], r
    r["claims"] = alex.say("/claims")
    assert any("Your chunks (1 of 4)" in l for l in r["claims"]) and any("Trusted: Steve" in l for l in r["claims"]), r
    return r


def world_guard(server, alex, steve):
    """Nothing reaches Alex's chunk from outside: water, fire, a piston, TNT, Steve's bucket and fists."""
    r = {}
    _, (ax, ay, az), _ = where(alex)
    x0, z0, y = (math.floor(ax) >> 4) * 16, (math.floor(az) >> 4) * 16, math.floor(ay)
    r["untrust"] = alex.say("/untrust Steve")
    block = lambda x, yy, z: json.dumps(steve.run("block", x, yy, z))
    for line in ("weather clear", f"tp Alex {x0 + 8.5} {y} {z0 - 12.5}", f"tp Steve {x0 + 4.5} {y} {z0 - 1.5}",
                 # Water two blocks east of the chunk; fire on netherrack against planks on both sides of its
                 # west edge (more planks outside, so some burn soon).
                 f"setblock {x0 + 17} {y} {z0 + 8} minecraft:water",
                 f"setblock {x0 - 1} {y - 1} {z0 + 8} minecraft:netherrack",
                 f"setblock {x0} {y} {z0 + 8} minecraft:oak_planks",
                 f"setblock {x0 - 2} {y} {z0 + 8} minecraft:oak_planks",
                 f"setblock {x0 - 1} {y} {z0 + 7} minecraft:oak_planks",
                 f"setblock {x0 - 1} {y} {z0 + 9} minecraft:oak_planks",
                 f"setblock {x0 - 1} {y + 1} {z0 + 8} minecraft:oak_planks",
                 f"setblock {x0 - 1} {y} {z0 + 8} minecraft:fire",
                 # North of the chunk: a piston pushing dirt into it, another pushing away from it.
                 f"setblock {x0 + 8} {y} {z0 - 2} minecraft:piston[facing=south]",
                 f"setblock {x0 + 8} {y} {z0 - 1} minecraft:dirt",
                 f"setblock {x0 + 12} {y} {z0 - 2} minecraft:piston[facing=north]",
                 f"setblock {x0 + 12} {y} {z0 - 3} minecraft:dirt",
                 f"setblock {x0 + 8} {y} {z0 - 3} minecraft:redstone_block",
                 f"setblock {x0 + 12} {y} {z0 - 1} minecraft:redstone_block",
                 "give Steve minecraft:water_bucket",
                 f"summon minecraft:cow {x0 + 5.5} {y} {z0 + 0.5} {{NoAI:1b}}"):
        server.console(line)
    time.sleep(4)
    r["water"] = [block(x0 + 16, y, z0 + 8), block(x0 + 15, y, z0 + 8)]
    assert "water" in r["water"][0] and "water" not in r["water"][1], r
    r["pistons"] = [block(x0 + 8, y, z0 - 2), block(x0 + 8, y, z0 - 1), block(x0 + 12, y, z0 - 2)]
    assert "extended=false" in r["pistons"][0] and ("dirt" in r["pistons"][1] or "grass_block" in r["pistons"][1]), r
    assert "extended=true" in r["pistons"][2], r
    # Steve's bucket on the chunk's ground: no water; then on the ground outside it.
    steve.run("look-at-block", x0 + 3, y - 1, z0 + 1, "up")
    steve.run("use-item")
    time.sleep(1.5)
    r["bucket"] = [block(x0 + 3, y, z0 + 1), json.dumps(steve.run("messages", 5))]
    assert "water" not in r["bucket"][0] and "This land is claimed by Alex." in r["bucket"][1], r
    # Steve's fist on Alex's cow: nothing, until Alex trusts him.
    cow = steve.run("entities", 8, "cow")["entities"][0]
    steve.run("attack", cow["id"])
    time.sleep(1.5)
    r["cow"] = [steve.run("entities", 8, "cow")["entities"][0]["health"]]
    r["trust"] = alex.say("/trust Steve")
    steve.run("attack", cow["id"])
    time.sleep(1.5)
    r["cow"].append(steve.run("entities", 8, "cow")["entities"][0]["health"])
    assert r["cow"][0] == 10 and r["cow"][1] < 10, r
    # Fire: the planks outside burn, those inside never.
    for _ in range(90):
        outside = [block(x0 - 2, y, z0 + 8), block(x0 - 1, y, z0 + 7), block(x0 - 1, y, z0 + 9),
                   block(x0 - 1, y + 1, z0 + 8)]
        if sum("oak_planks" not in b for b in outside) >= 2:
            break
        time.sleep(1)
    r["fire"] = outside + [block(x0, y, z0 + 8)]
    assert sum("oak_planks" not in b for b in outside) >= 1 and "oak_planks" in r["fire"][-1], r
    # TNT two blocks south of the chunk: a crater outside, not one block of the chunk broken.
    inside = lambda: json.dumps(steve.run("blocks", x0 + 4, y - 4, z0 + 12, x0 + 12, y + 2, z0 + 15)["counts"])
    r["before_tnt"] = inside()
    server.console(f"summon minecraft:tnt {x0 + 8.5} {y} {z0 + 17.5} {{fuse:1}}")
    for _ in range(20):
        time.sleep(1)
        if "grass" not in block(x0 + 8, y - 1, z0 + 17):
            break
    time.sleep(1)
    r["after_tnt"] = [inside(), block(x0 + 8, y - 1, z0 + 17)]
    assert r["after_tnt"][0] == r["before_tnt"] and "grass" not in r["after_tnt"][1], r
    return r


def board(alex, steve, out):
    """The sidebar of the richest players and the Tab list; /sidebar off hides it, on shows it again."""
    r = {}
    side = lambda c: c.run("status").get("sidebar")
    for _ in range(20):
        r["sidebar"] = side(steve)
        if r["sidebar"]:
            break
        time.sleep(0.5)
    assert r["sidebar"]["objective"] == "horizon:native_sidebar" and r["sidebar"]["title"] == "Richest players", r
    # Alex and Steve, a gap, then Steve's own line.
    assert len(r["sidebar"]["scores"]) == 4, r
    # Holders starting with '#' would be hidden from the sidebar (only the title would show).
    assert not any(o.startswith("#") for o in r["sidebar"]["scores"]), r
    steve.run("key", "tab", "down")
    time.sleep(1)
    steve.run("screenshot", str(Path(out).resolve() / "sidebar-and-tab.png"))
    steve.run("key", "tab", "up")
    r["off"] = steve.say("/sidebar off")
    assert any("The sidebar is off" in l for l in r["off"]), r
    time.sleep(1)
    assert not side(steve), side(steve)
    assert side(alex), "only Steve's is gone"
    r["on"] = steve.say("/sidebar")
    assert any("The sidebar is on" in l for l in r["on"]), r
    for _ in range(10):
        if side(steve):
            break
        time.sleep(0.5)
    assert side(steve), "back within a second or two"
    steve.say("/sidebar off")
    return r


def trades(server, alex, steve, port, ctl, resources, root):
    """Every way out of a trade, then one that goes through. Alex: 10 diamonds; Steve: 5 emeralds, 5 torches."""
    results = {}
    items = lambda: (alex.count("minecraft:diamond"), alex.count("minecraft:emerald"),
                     steve.count("minecraft:diamond"), steve.count("minecraft:emerald"))
    assert items() == (10, 0, 0, 5), items()
    # Closing the menu.
    open_trade(alex, steve)
    alex.click(alex.slot_of("minecraft:diamond"))
    shown = steve.menu().get(THEIR_FIRST, {})
    assert (shown.get("item"), shown.get("count")) == ("minecraft:diamond", 10), shown
    assert alex.count("minecraft:diamond") == 10, "offered stacks stay in the inventory"
    alex.run("close")
    time.sleep(1.5)
    results["closed"] = steve.chat()[-2:]
    assert any("Trade cancelled: Alex closed the trade. Nothing was exchanged." in l for l in results["closed"])
    assert not steve.menu(), "the other's menu closes too"
    assert items() == (10, 0, 0, 5), items()
    # A full inventory: Steve can't take the diamonds; nothing moves, the trade stays open, both must confirm again.
    server.console("give Steve minecraft:dirt 2176")
    time.sleep(2)
    open_trade(alex, steve)
    alex.click(alex.slot_of("minecraft:diamond"))
    alex.click(CONFIRM)
    steve.click(CONFIRM)
    results["full"] = alex.chat()[-2:]
    assert any("The trade didn't go through: the second player has no room." in l for l in results["full"]), results
    assert alex.menu().get(45, {}).get("item") == "minecraft:lime_stained_glass_pane", "confirmations cleared"
    assert items() == (10, 0, 0, 5), items()
    # The cancel button.
    steve.click(CANCEL)
    assert any("Trade cancelled: Steve cancelled." in l for l in alex.chat()[-2:])
    server.console("clear Steve minecraft:dirt")
    time.sleep(1)
    # Through: Alex's diamonds for Steve's emeralds and 35 coins. Steve's money offer clears Alex's confirmation.
    open_trade(alex, steve)
    alex.click(alex.slot_of("minecraft:diamond"))
    steve.click(steve.slot_of("minecraft:emerald"))
    alex.click(CONFIRM)
    assert alex.menu()[45]["item"] == "minecraft:lime_concrete"
    assert steve.menu()[52]["item"] == "minecraft:lime_concrete", "Steve sees Alex confirmed"
    steve.say("/trade money 35")
    assert alex.menu()[45]["item"] == "minecraft:lime_stained_glass_pane", "a change clears the confirmation"
    alex.click(CONFIRM)
    steve.click(CONFIRM)
    time.sleep(1)
    results["done"] = alex.chat()[-2:]
    assert any("Trade with Steve done" in l for l in results["done"]), results
    assert items() == (0, 5, 10, 0), items()
    assert any("Balance: 415 coins" in l for l in alex.say("/balance"))
    assert any("Balance: 400 coins" in l for l in steve.say("/balance"))
    # Dying (inventories kept, so a duplicate would show): Alex's death ends it, nothing moved.
    server.console("gamerule keep_inventory true")
    time.sleep(1)
    open_trade(alex, steve)
    alex.click(alex.slot_of("minecraft:emerald"))
    server.console("kill Alex")
    time.sleep(2)
    results["death"] = steve.chat()[-2:]
    assert any("Trade cancelled" in l for l in results["death"]), results
    alex.run("respawn")
    time.sleep(2)
    assert items() == (0, 5, 10, 0), items()
    # Leaving: Steve quits mid-trade; nothing moved, his items are his when he comes back.
    open_trade(alex, steve)
    steve.click(steve.slot_of("minecraft:diamond"))
    steve.quit()
    time.sleep(3)
    results["left"] = alex.chat()[-2:]
    assert any("Trade cancelled" in l for l in results["left"]), results
    assert not alex.menu()
    steve = Client(ctl, resources, "Steve2", root)
    steve.name = "Steve"
    steve.join(port)
    assert items() == (0, 5, 10, 0), items()
    return results, steve


def run(ctl, resources, out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    game = Path(ctl).with_name("horizon-client")
    root = Path(tempfile.mkdtemp(prefix="smp-proof-"))
    port = free_port()
    server = Server(game, resources, root / "server", port)
    alex = Client(ctl, resources, "Alex", root)
    steve = Client(ctl, resources, "Steve", root)
    results = {}
    try:
        data = server.folder / "world" / "horizon-smp"
        data.mkdir(parents=True)
        (data / "config.json").write_text(json.dumps({"gambling": {"enabled": True, "cooldown_seconds": 20,
                                                                         "lottery_seconds": 30},
                                                      "places": {"warmup_seconds": 2, "cooldown_seconds": 0}}))
        server.start()
        # No monster may end a step early: a slime once killed Steve on the flat world.
        for line in ("difficulty peaceful", "gamerule spawn_monsters false", "kill @e[type=minecraft:slime]"):
            server.console(line)
        alex.join(port)
        steve.join(port)
        time.sleep(2)
        config = json.loads((data / "config.json").read_text())
        results["config_keys"] = sorted(config)
        assert results["config_keys"] == ["board", "claims", "currency", "gambling", "market", "places", "starting_balance"], config
        assert config["gambling"] == {"enabled": True, "min_bet": 10, "max_bet": 1000, "cooldown_seconds": 20,
                                      "offer_seconds": 60, "fee_percent": 0, "ticket_price": 10,
                                      "max_tickets": 100, "lottery_seconds": 30}, config
        assert config["market"]["items"]["minecraft:diamond"] == {"buy": 400, "sell": 100}, config
        server.console("op Alex")
        time.sleep(1)
        alex.say("/eco give Alex 500")
        results["balance"] = alex.say("/balance")
        assert any("Balance: 500 coins" in l for l in results["balance"]), results
        results["pay"] = alex.say("/pay Steve 120")
        assert any("You paid 120 coins to Steve. Balance: 380 coins" in l for l in results["pay"]), results
        results["paid"] = steve.chat()[-3:]
        assert any("Alex paid you 120 coins." in l for l in results["paid"]), results
        results["refused"] = [alex.say(l) for l in ("/pay Steve 9999", "/pay Alex 1", "/pay Nobody 1")]
        for lines, error in zip(results["refused"], ("You only have 380 coins.", "You can't pay yourself.",
                                                     "No player named Nobody has an account.")):
            assert any(error in l for l in lines), results["refused"]
        results["eco_refused"] = steve.say("/eco give Steve 1000")
        assert not any("now has" in l for l in results["eco_refused"]), results
        results["baltop"] = steve.say("/baltop")
        assert any("1. Alex: 380 coins" in l for l in results["baltop"]), results
        assert any("2. Steve: 120 coins" in l for l in results["baltop"]), results
        server.console("give Steve minecraft:diamond 3")
        server.console("give Steve minecraft:wheat 20")
        time.sleep(2)
        results["worth"] = steve.say("/worth diamond")
        assert any("diamond: the market pays 100 coins each, sells it for 400 coins each." in l
                   for l in results["worth"]), results
        results["sell_wheat"] = steve.say("/sell wheat")
        assert any("Sold 20 wheat for 20 coins." in l for l in results["sell_wheat"]), results
        results["sell_hand"] = steve.say("/sell hand")
        assert any("Sold 3 diamond for 300 coins." in l for l in results["sell_hand"]), results
        results["buy"] = steve.say("/buy torch 5")
        assert any("Bought 5 torch for 5 coins." in l for l in results["buy"]), results
        time.sleep(1)
        counts = steve.run("inventory").get("counts", {})
        results["steve_items"] = counts
        have = lambda item: counts.get(f"minecraft:{item}", counts.get(item, 0))
        assert (have("diamond"), have("wheat"), have("torch")) == (0, 0, 5), counts
        server.console("gamemode creative Alex")
        time.sleep(1)
        results["creative"] = alex.say("/sell all")
        assert any("only with players in survival or adventure mode" in l for l in results["creative"]), results
        server.console("gamemode survival Alex")
        server.console("give Alex minecraft:diamond 10")
        server.console("give Steve minecraft:emerald 5")
        time.sleep(2)
        results["trades"], steve = trades(server, alex, steve, port, ctl, resources, root)
        # Before the coinflips: their last offer must still wait when the server stops.
        results["places"] = places(server, alex, steve)
        results["claims"] = claims(server, alex, steve)
        results["world_guard"] = world_guard(server, alex, steve)
        results["board"] = board(alex, steve, out)
        results["coinflip"], alex_wins = coinflips(alex, steve)
        results["lottery"], alex_lotto = lottery(alex, steve, alex_wins)
        alex_after = 415 + (100 if alex_wins else -100) + (20 if alex_lotto else -30)
        # Steve's 50 is still waiting: the server stops with both on.
        lines = server.stop()
        steve.quit()
        alex.quit()
        assert not any("panicked" in l or "teleport failed" in l for l in lines), lines[-40:]
        saved = json.loads((data / "places.json").read_text())
        assert sorted(saved["warps"]) == ["shop"] and saved["spawn"] and len(saved["homes"]) == 1, saved
        saved = json.loads((data / "board.json").read_text())
        assert len(saved["hidden"]) == 1, saved
        saved = json.loads((data / "claims.json").read_text())
        assert len(saved["chunks"]) == 1 and len(saved["trust"]) == 1, saved
        economy = json.loads((data / "economy.json").read_text())
        balances = {a["name"]: a["balance"] for a in economy["accounts"].values()}
        results["economy.json"] = economy
        held = sum(economy.get("held", {}).values())
        assert balances["Alex"] == alex_after and sum(balances.values()) + held == 815, economy
        audit = [json.loads(l) for l in (data / "audit.log").read_text().splitlines()]
        results["audit"] = [a["kind"] for a in audit]
        assert results["audit"] == ["open", "open", "eco_give", "pay", "sell 20 minecraft:wheat",
                                    "sell 3 minecraft:diamond", "buy 5 minecraft:torch", "trade", "trade_failed",
                                    "trade", "trade_money", "coinflip_stake", "coinflip_stake", "coinflip_win",
                                    "coinflip_stake", "lottery_ticket", "lottery_ticket", "lottery_win"] + (
                                    ["coinflip_cancel"] * (held == 0)), results["audit"]
        swap = [a for a in audit if a["kind"] == "trade"][-1]
        assert (swap["a_items"], swap["b_items"], swap["b_money"]) == (
            [{"item": "minecraft:diamond", "count": 10, "plain": True}],
            [{"item": "minecraft:emerald", "count": 5, "plain": True}], 35), swap
        server.start()
        alex = Client(ctl, resources, "Alex2", root)
        alex.name = "Alex"
        alex.join(port)
        results["after_restart"] = alex.say("/balance")
        assert any(f"Balance: {alex_after} coins" in l for l in results["after_restart"]), results
        alex.quit()
        server.stop()
        economy = json.loads((data / "economy.json").read_text())
        assert "held" not in economy and sum(a["balance"] for a in economy["accounts"].values()) == 815, economy
        kinds = [json.loads(l)["kind"] for l in (data / "audit.log").read_text().splitlines()]
        assert kinds[-1] in ("held_returned", "coinflip_cancel"), kinds[-3:]
        results["stake_back"] = kinds[-1]
        print(json.dumps({"proof": "smp", "ok": True, **results}, indent=2))
    finally:
        (out / "server.log").write_text("\n".join(server.lines))
        for c in (alex, steve):
            c.quit()
        if server.__dict__.get("process") and server.process.poll() is None:
            server.process.kill()
        shutil.rmtree(root, ignore_errors=True)
