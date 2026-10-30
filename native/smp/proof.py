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
- after the server stops cleanly and starts again, the balances are the same, and audit.log holds every change.

Everything runs in fresh temporary folders (the server's, each client's home, launcher and XDG folders), no audio.
"""
import json
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
        time.sleep(1.5)
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
    server.console("gamerule keepInventory true")
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
        server.start()
        alex.join(port)
        steve.join(port)
        time.sleep(2)
        data = server.folder / "world" / "horizon-smp"
        config = json.loads((data / "config.json").read_text())
        results["config_keys"] = sorted(config)
        assert results["config_keys"] == ["currency", "market", "starting_balance"], config
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
        steve.quit()
        alex.quit()
        lines = server.stop()
        assert not any("panicked" in l for l in lines), lines[-40:]
        economy = json.loads((data / "economy.json").read_text())
        balances = {a["name"]: a["balance"] for a in economy["accounts"].values()}
        results["economy.json"] = balances
        assert balances == {"Alex": 415, "Steve": 400}, balances
        audit = [json.loads(l) for l in (data / "audit.log").read_text().splitlines()]
        results["audit"] = [a["kind"] for a in audit]
        assert results["audit"] == ["open", "open", "eco_give", "pay", "sell 20 minecraft:wheat",
                                    "sell 3 minecraft:diamond", "buy 5 minecraft:torch", "trade", "trade_failed",
                                    "trade", "trade_money"], results["audit"]
        swap = [a for a in audit if a["kind"] == "trade"][-1]
        assert (swap["a_items"], swap["b_items"], swap["b_money"]) == (
            [{"item": "minecraft:diamond", "count": 10, "plain": True}],
            [{"item": "minecraft:emerald", "count": 5, "plain": True}], 35), swap
        server.start()
        alex = Client(ctl, resources, "Alex2", root)
        alex.name = "Alex"
        alex.join(port)
        results["after_restart"] = alex.say("/balance")
        assert any("Balance: 415 coins" in l for l in results["after_restart"]), results
        alex.quit()
        server.stop()
        print(json.dumps({"proof": "smp", "ok": True, **results}, indent=2))
    finally:
        (out / "server.log").write_text("\n".join(server.lines))
        for c in (alex, steve):
            c.quit()
        if server.__dict__.get("process") and server.process.poll() is None:
            server.process.kill()
        shutil.rmtree(root, ignore_errors=True)
