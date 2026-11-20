#!/usr/bin/env python3
"""Check the weighted modes, height rounding, resend stability and dimension changes."""

import argparse
from collections import Counter
from contextlib import ExitStack
import json
from pathlib import Path
import tempfile

from dedicated import arguments, clients_for, server_for, unpack_state_ids


def run(args, folder, mode):
    from wire import CHUNK, Client, chunk_cell, column

    settings = f'''
[dimensions."minecraft:overworld"]
enabled = true
mode = {mode}
max_block_height = 15 # exclusive; rounds down to 0
hidden_blocks = ["air", "diamond_ore", "gold_ore", "gold_ore", "void_air"]
replacement_blocks = ["stone", "chest", "cave_air"]

[dimensions."minecraft:the_nether"]
enabled = true
hidden_blocks = ["ancient_debris"]
max_block_height = 128
'''
    server, port = server_for(args, folder, settings, flat=True)
    with ExitStack() as stack:
        stack.callback(server.stop)
        for command in (
            'gamerule spawn_mobs false',
            'gamerule respawn_radius 0',
            'setworldspawn 8 -60 8',
            'forceload add 0 0 15 15',
            'fill 0 -62 0 15 2 15 minecraft:stone',
            'setblock 8 -17 8 minecraft:diamond_ore',
            'setblock 11 -16 11 minecraft:chest',
            'execute in minecraft:the_nether run forceload add 0 0 15 15',
            'execute in minecraft:the_nether run fill 0 113 0 15 117 15 minecraft:netherrack',
            'execute in minecraft:the_nether run setblock 8 115 8 minecraft:ancient_debris',
            'execute if block 11 -16 11 minecraft:chest run say pattern fixture ready',
        ):
            server.console(command)
        server.wait_for('pattern fixture ready', 120)
        first, second = stack.enter_context(clients_for(Client, port, ('PatternA', 'PatternB')))
        states = unpack_state_ids(args.resources)
        stone = states('stone')
        decoys = {states('diamond_ore'), states('gold_ore')}
        chest = (11, -16, 11)

        def initial(client, mark=0):
            return client.wait(lambda p: p.kind == CHUNK and column(p) == (0, 0), mark)[1]

        def sample(packet):
            return [[chunk_cell(packet, (x, y, z))[0] for x in range(2, 7) for z in range(2, 7)]
                    for y in range(-60, -1)]

        start = initial(first)
        layers = sample(start)
        assert layers == sample(initial(second)), 'both protected players must share the same projection'
        assert all(state in decoys for layer in layers for state in layer)
        assert mode != 2 or any(len(set(layer)) > 1 for layer in layers)
        assert mode != 3 or all(len(set(layer)) == 1 for layer in layers)
        assert len(set().union(*map(set, layers))) == len(decoys), 'both decoys should appear'
        assert chunk_cell(start, (8, 0, 8))[0] == stone
        fake, tag = chunk_cell(start, chest)
        assert fake in decoys and tag is None

        mark = first.mark()
        server.console('tp PatternA 320.5 -60 8.5')
        first.wait(lambda p: p.kind == 38, mark)  # old columns were unloaded
        mark = first.mark()
        server.console('tp PatternA 8.5 -60 8.5')
        assert layers == sample(initial(first, mark)), 'resend changed a replacement pattern'

        mark = first.mark()
        server.console('execute in minecraft:the_nether run tp PatternA 8.5 118 8.5')
        respawn, _ = first.wait(lambda p: p.kind == 84, mark)
        nether = initial(first, respawn)
        assert chunk_cell(nether, (8, 115, 8), min_y=0)[0] == states('netherrack')
        mark = first.mark()
        server.console('execute in minecraft:overworld run tp PatternA 8.5 -60 8.5')
        respawn, _ = first.wait(lambda p: p.kind == 84, mark)
        assert layers == sample(initial(first, respawn))
        return {
            'mode': mode, 'decoy_state_counts': dict(Counter(value for layer in layers for value in layer)),
            'layers_with_more_than_one_decoy': sum(len(set(layer)) > 1 for layer in layers),
            'permission': 'protected', 'nether_projection': 'netherrack',
            'resend_stability_verified': True, 'maximum_y_exclusive': 0,
            'replacement_chest_tag_suppressed': True,
        }


def rejects_entity_decoy(args, folder):
    from wire import Client

    settings = '''
[dimensions."minecraft:overworld"]
enabled = true
mode = 2
hidden_blocks = ["chest"]
'''
    server, port = server_for(args, folder, settings, flat=True)
    try:
        server.wait_for(r'minecraft:overworld\.hidden_blocks.*minecraft:chest.*cannot be a decoy', 15)
        try:
            client = Client(port, 'InvalidConfig')
            try:
                client.wait(lambda p: p.kind == 73, timeout=10)
            finally:
                client.close()
        except (EOFError, OSError, RuntimeError) as error:
            if isinstance(error, TimeoutError):
                raise
            return
        raise AssertionError('invalid configuration allowed a login')
    finally:
        server.stop()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', type=int, choices=(2, 3), required=True)
    args = arguments(parser)
    folder = Path(tempfile.mkdtemp(dir=args.out, prefix=f'pattern-{args.mode}-'))
    result = run(args, folder, args.mode)
    if args.mode == 2:
        rejects_entity_decoy(args, Path(tempfile.mkdtemp(dir=args.out, prefix='invalid-')))
        result['invalid_decoy_rejected_at_startup'] = True
    (folder / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
