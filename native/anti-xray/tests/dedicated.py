#!/usr/bin/env python3
"""Run the mode 1 packet milestone against a native dedicated-server export."""

import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack, contextmanager
import json
import os
from pathlib import Path
import sys
import tempfile
import time


def connect_tools(engine):
    sys.path.insert(0, str(engine / 'tools'))


def server_for(args, folder, extra, *, flat=False, far_distance=None):
    import lab
    from server_bench import DONE, Server

    port = lab.free_port()
    properties = {
        'online-mode': 'false',
        'white-list': 'false',
        'server-ip': '127.0.0.1',
        'server-port': port,
        'level-seed': 87387,
        'gamemode': 'creative',
        'spawn-protection': 0,
        'view-distance': args.view_distance,
        'simulation-distance': 3,
        'network-compression-threshold': 256,
        'pause-when-empty-seconds': -1,
    }
    if flat:
        properties['level-type'] = 'minecraft:flat'
        properties['generate-structures'] = 'false'
    if far_distance is not None:
        properties['horizon-max-view-distance'] = far_distance
    (folder / 'server.properties').write_text(''.join(f'{k}={v}\n' for k, v in properties.items()))
    (folder / 'server.toml').write_text('[anticheat]\nenabled = false\n')
    (folder / 'eula.txt').write_text('eula=true\n')
    (folder / 'config').mkdir(exist_ok=True)
    (folder / 'config/anti_xray.toml').write_text(extra)
    env = os.environ.copy()
    for variable in ('HORIZON_HOME', 'HORIZON_LAUNCHER_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME',
                     'HORIZON_LAUNCHER_TRASH', 'HORIZON_LAUNCHER_CLIPBOARD', 'HORIZON_LAUNCHER_DESKTOP'):
        env[variable] = tempfile.mkdtemp(dir=folder, prefix=variable.lower() + '-')
    server = Server([args.binary, '--game-dir', folder, '--resources', args.resources, '--dedicated', '--nogui'], folder, folder / 'console.log', env)
    try:
        server.wait_for(DONE, 300)
    except BaseException:
        server.stop()
        raise
    return server, port


SCENE = '''
[dimensions."minecraft:overworld"]
enabled = true
hidden_blocks = ["redstone_ore", "chest"]
max_block_height = 64
permission_bypass = true
'''

ORE = (15, -55, 1)
CHEST = (16, -55, 0)
UPPER_ORE = (9, 5, 4)
SEAL = (15, -55, 0)


def unpack_state_ids(resources):
    facts = json.loads((resources / 'facts.json').read_text())['states']
    definitions = json.loads((resources / 'render-definitions.json').read_text())
    def state(name, **properties):
        definition = definitions[f'minecraft:{name}']
        if not properties:
            return definition['default']
        matches = [idx for idx in definition['states']
                   if facts[idx][4].get('properties', {}).items() >= properties.items()]
        if len(matches) != 1:
            raise ValueError(f'expected one {name} state for {properties}, got {matches}')
        return matches[0]
    return state


@contextmanager
def clients_for(client_type, port, names):
    with ExitStack() as stack:
        with ThreadPoolExecutor(len(names)) as pool:
            jobs = [pool.submit(client_type, port, name) for name in names]
            clients, error = [], None
            for job in jobs:
                try:
                    client = job.result()
                    clients.append(client)
                    stack.callback(client.close)
                except Exception as failure:
                    error = failure
            if error is not None:
                raise error
        yield clients


def run(args, folder):
    import lab
    from wire import ACK, CHUNK, Client, Reader, chunk_cell, column, tag_at, updated_state

    # Use the UUID that the unauthenticated loopback client will present.
    (folder / 'ops.json').write_text(json.dumps([{
        'uuid': str(lab.offline_uuid('Operator')), 'name': 'Operator', 'level': 4,
        'bypassesPlayerLimit': False,
    }]))
    cfg = SCENE + f'lava_obscures = {str(args.lava_obscures).lower()}\n'
    server, port = server_for(args, folder, cfg, flat=True)
    with ExitStack() as stack:
        stack.callback(server.stop)
        for command in (
            'forceload add 0 -16 31 15',
            'setworldspawn 12 -60 -3',
            'gamerule respawn_radius 0',
            'gamerule random_tick_speed 0',
            'gamerule spawn_mobs false',
            'fill 12 -57 0 20 -53 6 minecraft:stone',
            'setblock 16 -55 -1 minecraft:stone',
            'setblock 15 -55 1 minecraft:redstone_ore[lit=true]',
            'setblock 16 -55 0 minecraft:chest[facing=east]{Items:[{Slot:0b,id:"minecraft:diamond",count:3}]}',
            'fill 1 -57 1 12 -53 7 minecraft:stone',
            'setblock 2 -55 3 minecraft:redstone_ore',
            'setblock 5 -55 3 minecraft:redstone_ore',
            'setblock 8 -55 3 minecraft:redstone_ore',
            'setblock 11 -55 3 minecraft:redstone_ore',
            'setblock 2 -55 2 minecraft:glass',
            'setblock 5 -55 2 minecraft:stone_slab',
            'setblock 8 -55 2 minecraft:water',
            'setblock 11 -55 2 minecraft:lava',
            'fill 8 4 3 10 6 5 minecraft:stone',
            'setblock 9 5 4 minecraft:redstone_ore',
            'execute if block 16 -55 0 minecraft:chest[facing=east] run say anti-xray fixture ready',
        ):
            server.console(command)
        server.wait_for('anti-xray fixture ready', 45)
        clients = stack.enter_context(clients_for(Client, port, ('ProtectedA', 'ProtectedB', 'Operator')))
        state = unpack_state_ids(args.resources)
        cover = state('deepslate')
        real_ore = state('redstone_ore', lit='true')
        real_chest = state('chest', facing='east', type='single', waterlogged='false')

        def view(client, position, expected, since=0):
            return client.wait(lambda p: p.kind == CHUNK and column(p) == (position[0] >> 4, position[2] >> 4)
                               and chunk_cell(p, position)[0] == expected, since)

        summary = {}
        for client in clients:
            bypassed = client.name == 'Operator'
            states = []
            for position, expected in ((ORE, real_ore), (CHEST, real_chest)):
                index, packet = view(client, position, expected if bypassed else cover)
                if not bypassed:
                    for prior in client.packets[:index]:
                        if prior.kind == CHUNK and column(prior) == column(packet):
                            assert chunk_cell(prior, position) == (cover, None), client.name
                value, tag = chunk_cell(packet, position)
                assert bool(tag) == (bypassed and position == CHEST), (client.name, position, tag)
                states.append({'position': position, 'state': value, 'embedded_tag': tag is not None})
            upper = state('redstone_ore' if bypassed else 'stone')
            view(client, UPPER_ORE, upper)
            summary[client.name] = states + [{'position': UPPER_ORE, 'state': upper, 'embedded_tag': False}]
            if not bypassed:
                for x in (2, 5, 8, 11):
                    expected = cover if x == 11 and args.lava_obscures else state('redstone_ore')
                    view(client, (x, -55, 3), expected)

        first = clients[0]
        mark = first.mark()
        server.console('tp ProtectedA 0.5 -55 -2.5')
        first.wait(lambda p: p.kind == 73, since=mark)
        marks = [c.mark() for c in clients]
        first.dig(ORE, 1)
        first.wait(lambda p: p.kind == ACK and Reader(p.data).varint() >= 1, marks[0])
        for client, mark in zip(clients[:2], marks):
            assert not any(updated_state(p, ORE) == real_ore or tag_at(p, CHEST)
                           for p in client.packets[mark:]), client.name
        for client in clients[:2]:
            mark = client.mark()
            server.console(f'tp {client.name} 15.5 -55 -2.0')
            client.wait(lambda p: p.kind == 73, mark)
        marks = [c.mark() for c in clients]
        first.dig(SEAL, 2)
        tag_order = {}
        for client, mark in zip(clients[:2], marks):
            client.wait(lambda p: updated_state(p, ORE) == real_ore, mark)
            index, _ = client.wait(lambda p: updated_state(p, CHEST) == real_chest, mark)
            try:
                tag_index, _ = client.wait(lambda p: tag_at(p, CHEST) is not None, mark, timeout=1.0)
            except TimeoutError:
                if args.require_chest_tag:
                    raise
                tag_order[client.name] = 'missing'
            else:
                assert index < tag_index, (client.name, index, tag_index)
                tag_order[client.name] = 'state before tag'
        summary['chest_reveal_tag'] = tag_order

        marks = [c.mark() for c in clients]
        server.console('setblock 15 -55 0 minecraft:stone')
        for client, mark in zip(clients[:2], marks):
            client.wait(lambda p: updated_state(p, ORE) == cover, mark)
            client.wait(lambda p: updated_state(p, CHEST) == cover, mark)
        op = clients[2]
        mark = op.mark()
        server.console('deop Operator')
        _, protected = view(op, CHEST, cover, mark)
        assert chunk_cell(protected, CHEST) == (cover, None)
        mark = op.mark()
        server.console('op Operator')
        _, bypassed = view(op, CHEST, real_chest, mark)
        assert chunk_cell(bypassed, CHEST)[1] is not None

        def chest_state(packet):
            if packet.kind == CHUNK and column(packet) == (CHEST[0] >> 4, CHEST[2] >> 4):
                return chunk_cell(packet, CHEST)[0]
            return updated_state(packet, CHEST)

        for material, expected in (('air', real_chest), ('stone', cover)):
            marks = [client.mark() for client in clients[:2]]
            server.console(f'fill 15 -55 0 15 -54 0 minecraft:{material}')
            for client, mark in zip(clients[:2], marks):
                client.wait(lambda p: chest_state(p) == expected, mark)

        server.console('data get block 16 -55 0 Items')
        server.wait_for('minecraft:diamond.*count: ?3|count: ?3.*minecraft:diamond', 20)
        server.console('save-all flush')
        server.wait_for('Saved the game', 45)
        assert_saved_fixture(folder)
        server.console('setblock 16 -55 0 minecraft:air destroy')
        server.console('execute if entity @e[type=minecraft:item,nbt={Item:{id:"minecraft:diamond",count:3}}] run say anti-xray authoritative drop')
        server.wait_for('anti-xray authoritative drop', 20)

        # Mine the real redstone ore in survival, using a vanilla dig and the proper tool.
        mark = first.mark()
        for command in (
            'setblock 15 -55 0 minecraft:air',
            'setblock 15 -56 -1 minecraft:stone',
            'item replace entity ProtectedA hotbar.0 with minecraft:diamond_pickaxe',
            'gamemode survival ProtectedA',
            'tp ProtectedA 15.5 -55 -0.5',
        ):
            server.console(command)
        first.wait(lambda p: p.kind == 73, mark)
        mark = first.mark()
        first.dig(ORE, 3)
        first.wait(lambda p: p.kind == ACK and Reader(p.data).varint() >= 3, mark, timeout=5)
        first.dig(ORE, 4, action=3)  # STOP_DESTROY_BLOCK lets the server finish the dig.
        air = state('air')
        first.wait(lambda p: updated_state(p, ORE) == air, mark, timeout=8)
        server.console('execute if entity @e[type=minecraft:item,nbt={Item:{id:"minecraft:redstone"}}] run say anti-xray redstone drop')
        server.wait_for('anti-xray redstone drop', 20)
        summary['checks'] = ['opaque full cubes', 'glass', 'single slab', 'water', 'lava',
                             'out-of-range interaction', 'accepted mining',
                             'reseal', 'live deop and op', 'fill edit', 'authoritative saved ore and chest contents', 'chest drop', 'survival ore drop']
        summary['lava_obscures'] = args.lava_obscures
        return summary


def assert_saved_fixture(folder):
    from anvil_dump import plain, region_chunks

    path = folder / 'world/dimensions/minecraft/overworld/region/r.0.0.mca'
    chunks = region_chunks(path)

    def saved(position):
        x, y, z = position
        chunk = chunks[x >> 4, z >> 4]
        section = next(row for row in chunk['sections'] if plain(row['Y'], True) == y >> 4)
        blocks = section['block_states']
        palette = blocks['palette']
        if len(palette) == 1:
            return palette[0]
        bits = max(4, (len(palette) - 1).bit_length())
        index = (y & 15) * 256 + (z & 15) * 16 + (x & 15)
        word, offset = divmod(index, 64 // bits)
        return palette[(blocks['data'][1][word] >> (offset * bits)) & ((1 << bits) - 1)]

    assert saved(ORE)['id'] == 'minecraft:redstone_ore'
    assert saved(CHEST)['id'] == 'minecraft:chest'
    x, y, z = CHEST
    tags = chunks[x >> 4, z >> 4]['block_entities']
    tag = next(tag for tag in tags if (tag['x'], tag['y'], tag['z']) == CHEST)
    assert {'Slot': 0, 'id': 'minecraft:diamond', 'count': 3} in plain(tag['Items'], True)


def arguments(parser):
    parser.add_argument('--engine', required=True, type=Path)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--resources', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--view-distance', default=6, type=int)
    args = parser.parse_args()
    for field in ('engine', 'binary', 'resources', 'out'):
        setattr(args, field, getattr(args, field).resolve())
    args.out.mkdir(parents=True, exist_ok=True)
    connect_tools(args.engine)
    return args


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lava-obscures', action='store_true')
    parser.add_argument('--require-chest-tag', action='store_true')
    args = arguments(parser)
    folder = Path(tempfile.mkdtemp(dir=args.out, prefix='packet-'))
    result = run(args, folder)
    (folder / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
