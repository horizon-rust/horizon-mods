#!/usr/bin/env python3
"""Read a previously saved column outside the dedicated server's resident view."""

import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import shutil
import tempfile

from dedicated import arguments, clients_for, server_for, unpack_state_ids

POSITION = (200, -52, 8)
KEY = (12, 0)
CONFIG = '''
[dimensions."minecraft:overworld"]
enabled = true
hidden_blocks = ["chest"]
permission_bypass = true
'''


def run(args, root):
    import lab
    from wire import CHUNK, Client, chunk_cell, column

    prep = root / 'prepare'
    prep.mkdir()
    server, port = server_for(args, prep, '', flat=True)
    with ExitStack() as stack:
        stack.callback(server.stop)
        client = stack.enter_context(clients_for(Client, port, ('Builder',)))[0]
        client.wait(lambda p: p.kind == 73)
        mark = client.mark()
        server.console('tp Builder 200.5 -60 8.5')
        client.wait(lambda p: p.kind == CHUNK and column(p) == KEY, mark)
        for command in (
            'fill 197 -54 5 203 -50 11 minecraft:stone',
            'setblock 200 -52 8 minecraft:chest',
            'setworldspawn 0 -60 0',
            'gamerule respawn_radius 0',
            'execute if block 200 -52 8 minecraft:chest run say saved-ring fixture ready',
        ):
            server.console(command)
        server.wait_for('saved-ring fixture ready', 30)

    verify = root / 'verify'
    verify.mkdir()
    shutil.copytree(prep / 'world', verify / 'world')
    (verify / 'ops.json').write_text(json.dumps([{
        'uuid': str(lab.offline_uuid('RingOperator')), 'name': 'RingOperator', 'level': 4,
        'bypassesPlayerLimit': False,
    }]))
    server, port = server_for(args, verify, CONFIG, flat=True, far_distance=15)
    with ExitStack() as stack:
        stack.callback(server.stop)
        server.wait_for('Sending saved chunks up to 15 chunks away', 20)
        result = {}
        protected, bypassed = stack.enter_context(clients_for(Client, port, ('RingProtected', 'RingOperator')))
        states = unpack_state_ids(args.resources)
        for client, expected, tag in ((protected, states('deepslate'), False),
                                      (bypassed, states('chest'), True)):
            _, packet = client.wait(lambda p: p.kind == CHUNK and column(p) == KEY)
            state, data = chunk_cell(packet, POSITION)
            assert (state, data is not None) == (expected, tag), (client.name, state, data)
            assert client.position[0] < 16.0 and client.position[2] < 16.0, client.position
            result[client.name] = {'column': KEY, 'state': state, 'embedded_tag': data is not None}
    return result


def main():
    args = arguments(argparse.ArgumentParser(description=__doc__))
    if args.view_distance >= KEY[0] - 2:
        raise ValueError('view distance must leave the fixture outside the resident view')
    root = Path(tempfile.mkdtemp(dir=args.out, prefix='far-'))
    result = run(args, root)
    (root / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
