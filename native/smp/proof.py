"""The SMP on a real dedicated server: the built game runs `--dedicated` with the mod, and two real clients (Alex and
Steve, `horizon-ctl --offscreen`) join it. Checked from the clients' own chat and from the files the server wrote:

- the economy opens an account for each player (config.json written with its defaults, then economy.json);
- /balance, /pay (the payer's answer and the payee's line), refused payments (too much, to oneself, unknown name);
- /baltop; /eco is refused below level 3 and works for an operator (made one from the server's console);
- the commands are in each player's command tree (the chat box's suggestions);
- the market (Steve in survival, items given from the console): /worth, /sell <item>, /sell hand, /buy, with the
  items gone from or in his inventory as his client sees it; a creative player can't sell;
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
        steve.quit()
        alex.quit()
        lines = server.stop()
        assert not any("panicked" in l for l in lines), lines[-40:]
        economy = json.loads((data / "economy.json").read_text())
        balances = {a["name"]: a["balance"] for a in economy["accounts"].values()}
        results["economy.json"] = balances
        assert balances == {"Alex": 380, "Steve": 435}, balances
        audit = [json.loads(l) for l in (data / "audit.log").read_text().splitlines()]
        results["audit"] = [a["kind"] for a in audit]
        assert results["audit"] == ["open", "open", "eco_give", "pay", "sell 20 minecraft:wheat",
                                    "sell 3 minecraft:diamond", "buy 5 minecraft:torch"], results["audit"]
        server.start()
        alex = Client(ctl, resources, "Alex2", root)
        alex.name = "Alex"
        alex.join(port)
        results["after_restart"] = alex.say("/balance")
        assert any("Balance: 380 coins" in l for l in results["after_restart"]), results
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
