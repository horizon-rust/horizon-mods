"""Recipes in a running modded game: the WebAssembly mod's setting is read and written as it was, the panel
beside a crafting table's screen shows items (container screens show it; the creative inventory doesn't), a
click on one gives it in creative mode, and with the setting off the same click opens its recipe page."""
import json
import time
from game import Game, difference

DATA = "horizon-mods/.horizon-mod-data/recipes"
# A player's setting from the WebAssembly mod: "Click gives items" on.
FILES = {f"{DATA}/give": "true"}
SLOT = 18


def first_item(g):
    """The panel's first item (Stone, the first item of the registry): its centre in GUI pixels, and the panel's
    part of the screen."""
    ui = g.run("ui")
    width = ui["gui_size"][0]
    stone = next(w for w in ui["widgets"] if w.get("label") == "Stone" and w.get("kind") == "button")
    x, y, w, h = stone["bounds"]
    return x + w / 2, y + h / 2, ((x - 2 * SLOT) / width, 0.0, 1.0, 1.0)


def table(g):
    """Opens the crafting table placed beside the player."""
    x, y, z = (int(c // 1) for c in g.run("status")["player"]["position"])
    if not getattr(g, "table", None):
        g.run("command", f"setblock {x + 2} {y} {z} minecraft:crafting_table")
        g.table = (x + 2, y, z)
        time.sleep(1)
    g.run("interact", *map(str, g.table))
    time.sleep(1)
    assert g.run("screen").get("screen") not in ("playing", "creative"), g.run("screen")


def run(ctl, resources, out):
    with Game(ctl, resources, out, FILES) as g:
        g.run("options", "mods")
        assert "Recipes 2.0.0: ON" in g.labels(), g.labels()
        g.run("click", g.ids()["Settings"])
        page = g.labels()
        assert "Click gives items (creative): ON" in page, page
        g.play()
        g.run("command", "clear @s")
        table(g)
        x, y, panel = first_item(g)
        shown = g.shot("1-panel")
        g.run("click-at", str(x), str(y))
        time.sleep(1)
        given = g.run("inventory").get("counts", {})
        assert given, "a click on the panel's first item gave nothing"
        g.play()
        g.run("options", "mods")
        g.run("click", g.ids()["Settings"])
        g.run("click", g.ids()["Click gives items (creative): ON"])
        stored = (g.home / DATA / "give").read_text()
        g.play()
        table(g)
        before = g.shot("2-items")
        g.run("click-at", str(x), str(y))
        time.sleep(1)
        after = g.shot("3-recipe-page")
        results = {
            "given": given,
            "give_file_after_click": stored,
            "diff_panel": round(difference(before, after, panel), 1),
            "diff_panel_first_shot": round(difference(shown, before, panel), 1),
        }
        assert stored == "false", results
        assert results["diff_panel"] > 4, results
        print(json.dumps({"proof": "recipes", "ok": True, **results}, indent=2))
