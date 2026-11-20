#!/usr/bin/env python3
"""Fill the pinned SDK's retained-radius budget while two clients stay near the same chunks."""

import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import tempfile

from dedicated import arguments, clients_for, server_for, unpack_state_ids

CONFIG = '''
[dimensions."minecraft:overworld"]
enabled = true
hidden_blocks = ["diamond_ore", "chest"]
update_radius = 2
'''


def run(args, folder):
    from wire import ACK, BLOCK_ENTITY, CHUNK, Client, Reader, chunk_cell, column, tag_at, updated_state

    server, port = server_for(args, folder, CONFIG, flat=True)
    ore = unpack_state_ids(args.resources)('diamond_ore')
    air = unpack_state_ids(args.resources)('air')
    chest = unpack_state_ids(args.resources)('chest')
    fake = unpack_state_ids(args.resources)('deepslate')
    points = [(5 + 5 * x, -50, 5 + 5 * z) for z in range(8) for x in range(9)]
    early = (points[0][0] + 2, -50, points[0][2])
    late = (points[-1][0] + 2, -50, points[-1][2])
    with ExitStack() as stack:
        stack.callback(server.stop)
        for command in (
            'gamerule spawn_mobs false',
            'gamerule respawn_radius 0',
            'setworldspawn 20 -60 20',
            'forceload add 0 0 63 47',
            'fill 2 -53 2 48 -47 43 minecraft:diamond_ore',
            f'setblock {early[0]} {early[1]} {early[2]} minecraft:chest',
            f'setblock {late[0]} {late[1]} {late[2]} minecraft:chest',
            f'execute if block {late[0]} {late[1]} {late[2]} minecraft:chest run say capacity fixture ready',
        ):
            server.console(command)
        server.wait_for('capacity fixture ready', 90)
        actor, observer = stack.enter_context(clients_for(Client, port, ('Miner', 'Observer')))
        chunk_columns = {(x >> 4, z >> 4) for x, _, z in [*points, early, late]}
        for client in (actor, observer):
            for key in chunk_columns:
                client.wait(lambda p: p.kind == CHUNK and column(p) == key)
            for position in (early, late):
                _, packet = client.wait(lambda p: p.kind == CHUNK
                                        and column(p) == (position[0] >> 4, position[2] >> 4))
                assert chunk_cell(packet, position) == (fake, None), client.name

        extra = ordinary = 0
        cap_reached_after = None
        chest_results = {}
        for seq, (x, y, z) in enumerate(points, 1):
            position = (x, y, z)
            mark = actor.mark()
            # Remain within a few chunks of (20,20); the tracked area includes all sites.
            server.console(f'tp Miner {x + 0.5} {y - 1} {z - 1.5}')
            actor.wait(lambda p: p.kind == 73, mark)
            actor_mark, observer_mark = actor.mark(), observer.mark()
            actor.dig(position, seq)
            actor.wait(lambda p: updated_state(p, position) == air, actor_mark)
            # Block deltas precede the next sequenced mining request. Observe both streams.
            observer.wait(lambda p: updated_state(p, position) == air, observer_mark)
            actor.wait(lambda p: p.kind == ACK and Reader(p.data).varint() >= seq, actor_mark)

            second_ring = [(x + dx, y + dy, z + dz)
                           for dx in range(-2, 3) for dy in range(-2, 3) for dz in range(-2, 3)
                           if abs(dx) + abs(dy) + abs(dz) == 2]
            ordinary_cell = (x - 1, y, z)
            actor.wait(lambda p: updated_state(p, ordinary_cell) == ore, actor_mark)
            observer.wait(lambda p: updated_state(p, ordinary_cell) == ore, observer_mark)
            sent, watched = actor.packets[actor_mark:], observer.packets[observer_mark:]
            assert any(updated_state(p, ordinary_cell) == ore for p in sent)
            ordinary += 1
            count = 0
            for target in second_ring:
                expected = chest if target in (early, late) else ore
                updates = [i for i, packet in enumerate(sent) if updated_state(packet, target) == expected]
                if updates:
                    count += 1
                assert not any(updated_state(packet, target) == expected for packet in watched), target
                if target in (early, late):
                    tags = [i for i, p in enumerate(sent) if p.kind == BLOCK_ENTITY and tag_at(p, target)]
                    if tags:
                        assert updates and updates[0] < tags[0]
                    chest_results[str(target)] = {'state': bool(updates), 'tag': bool(tags)}
            extra += count
            if extra == 1024 and cap_reached_after is None:
                cap_reached_after = seq

        # At capacity a chest can still become ordinarily exposed by breaking its immediate seal.
        x, y, z = late
        mark = actor.mark()
        observer_mark = observer.mark()
        actor.dig((x - 1, y, z), len(points) + 1)
        index, _ = actor.wait(lambda p: updated_state(p, late) == chest, mark)
        observer.wait(lambda p: updated_state(p, late) == chest, observer_mark)
        try:
            tag_index, _ = actor.wait(lambda p: tag_at(p, late) is not None, index, timeout=1)
        except TimeoutError:
            tag_after_capacity = False
        else:
            assert index < tag_index
            tag_after_capacity = True
        assert ordinary == len(points)
        assert extra == 1024, (extra, cap_reached_after, chest_results)
        assert chest_results[str(early)]['state'] and not chest_results[str(late)]['state']
        return {
            'accepted_breaks': len(points) + 1, 'ordinary_ore_reveals_verified': ordinary,
            'retained_radius_positions': extra, 'cap_reached_after_break': cap_reached_after,
            'radius_chest_results': chest_results,
            'late_ordinary_chest': {'state': True, 'tag': tag_after_capacity},
        }


def main():
    args = arguments(argparse.ArgumentParser(description=__doc__))
    folder = Path(tempfile.mkdtemp(dir=args.out, prefix='sustained-'))
    result = run(args, folder)
    (folder / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
