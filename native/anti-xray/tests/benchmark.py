#!/usr/bin/env python3
"""Compare delivered terrain and server time with the same export and saved world."""

import argparse
from argparse import Namespace
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import time

from dedicated import arguments, server_for, unpack_state_ids


class Deliveries:
    def __init__(self):
        self.chunks = []

    def receive(self, packet):
        from wire import column
        self.chunks.append((packet.at, column(packet), packet.wire_bytes))


def prepare_snapshot(args, output):
    folder = output / 'snapshot'
    folder.mkdir()
    server, _ = server_for(args, folder, '')
    try:
        server.console('gamerule spawn_mobs false')
        server.console('gamerule random_tick_speed 0')
        server.console('gamerule respawn_radius 0')
        server.console('forceload add -16 -16 63 63')
        server.console('setworldspawn 0 200 0')
        # A controlled workspace above the natural terrain, below the configured height cut-off.
        # The tunnels are reached by teleporting, so no terrain or loot is edited during exploration.
        server.console(f'fill 0 94 0 63 97 {4 * args.bots} minecraft:stone')
        for i in range(args.bots):
            z = 1 + i * 4
            server.console(f'fill 1 95 {z} 62 96 {z} minecraft:air')
            server.console(f'fill 1 95 {z + 2} 62 95 {z + 2} minecraft:diamond_ore')
        server.console(f'execute if block 62 95 {4 * args.bots - 1} minecraft:diamond_ore run say benchmark world ready')
        server.wait_for('benchmark world ready', 90)
    finally:
        server.stop()
    return folder / 'world'


def proc_usage(pid):
    process = Path('/proc') / str(pid)
    ticks = (process / 'stat').read_text().rsplit(')', 1)[1].split()
    cpu_s = (int(ticks[11]) + int(ticks[12])) / os.sysconf('SC_CLK_TCK')
    rss = int((process / 'statm').read_text().split()[1]) * os.sysconf('SC_PAGE_SIZE')
    return rss, cpu_s


def window(server, trackers, name, duration, workload):
    from server_bench import tick_query

    began = time.monotonic()
    _, initial_cpu = proc_usage(server.process.pid)
    end = began + duration
    samples = []
    peak = 0
    next_sample = began + min(2.5, duration / 2)
    while time.monotonic() < end:
        now = time.monotonic()
        workload(now - began)
        rss, _ = proc_usage(server.process.pid)
        peak = max(peak, rss)
        if now >= next_sample:
            if stats := tick_query(server):
                samples.append(stats)
            next_sample += 5
        time.sleep(max(0, min(0.05, end - time.monotonic())))
    _, final_cpu = proc_usage(server.process.pid)
    by_client = [tracker.chunks[:] for tracker in trackers]
    windows = [[row for row in rows if began <= row[0] < end] for rows in by_client]
    delivered = [row for rows in windows for row in rows]
    prior = [{column for at, column, _ in rows if at < began} for rows in by_client]
    unique_by_client = [{row[1] for row in rows} for rows in windows]
    unique = sum(map(len, unique_by_client))
    new = sum(len(columns - seen) for columns, seen in zip(unique_by_client, prior))
    return {
        'phase': name, 'seconds': duration, 'delivered_chunks': len(delivered),
        'delivered_chunks_per_second': len(delivered) / duration,
        'distinct_chunks_per_second': unique / duration,
        'first_time_chunks_per_second': new / duration,
        'repeated_deliveries_within_window': len(delivered) - unique,
        'compressed_framed_bytes_per_chunk': statistics.fmean(row[2] for row in delivered) if delivered else None,
        'server_cpu_seconds': final_cpu - initial_cpu,
        'sampled_peak_server_rss_bytes': peak,
        'tick_query_samples_ms': samples,
        'p95_ms': statistics.median(sample['p95'] for sample in samples) if samples else None,
        'p99_ms': statistics.median(sample['p99'] for sample in samples) if samples else None,
    }


def one_trial(args, output, snapshot, mode, round_number):
    from wire import ACK, Client, Reader, updated_state

    folder = output / f'round-{round_number}-mode-{mode}'
    folder.mkdir()
    shutil.copytree(snapshot, folder / 'world')
    server, port = server_for(args, folder, config_for(mode))
    with ExitStack() as stack:
        stack.callback(server.stop)
        trackers = [Deliveries() for _ in range(args.bots)]
        clients = []

        with ThreadPoolExecutor(args.bots) as pool:
            pending = []

            def start_join(_):
                if not pending:
                    pending.extend(pool.submit(Client, port, f'Bench{i}', tracker.receive)
                                   for i, tracker in enumerate(trackers))

            rows = [window(server, trackers, 'join', args.seconds, start_join)]
            for future in pending:
                client = future.result()
                stack.callback(client.close)
                client.wait(lambda p: p.kind == 73)
                clients.append(client)

        for client in clients:
            mark = client.mark()
            server.console(f'gamemode spectator {client.name}')
            server.console(f'tp {client.name} 0.5 200 0.5')
            client.wait(lambda p: p.kind == 73, mark)
        origins = [client.position for client in clients]
        directions = [(math.cos(math.tau * i / len(clients)), math.sin(math.tau * i / len(clients)))
                      for i in range(len(clients))]

        latest_move = 0.0

        def travel(seconds):
            nonlocal latest_move
            now = time.monotonic()
            if now - latest_move < 0.15:
                return
            latest_move = now
            for client, (x, _, z), (dx, dz) in zip(clients, origins, directions):
                distance = args.speed * max(0, seconds)
                client.move(x + dx * distance, 200, z + dz * distance)

        rows.append(window(server, trackers, 'cold', args.seconds, travel))
        if args.cold_only:
            return {'round': round_number, 'mode': mode, 'phases': rows}
        rows.append(window(server, trackers, 'warm', args.seconds,
                           lambda seconds: travel(args.seconds - seconds)))

        ore = unpack_state_ids(args.resources)('diamond_ore')
        batches = {client.name: [] for client in clients}
        events = []
        for i, client in enumerate(clients):
            mark = client.mark()
            server.console(f'gamemode creative {client.name}')
            z = 1 + i * 4
            server.console(f'tp {client.name} 1.5 95 {z + 0.5}')
            client.wait(lambda p: p.kind == 73, mark)
            batches[client.name] = [(x, z) for x in range(1, 62, 2)]
        next_break = 0.0
        interval = max(0.15, (args.seconds - 1) / 31)

        def mine(seconds):
            nonlocal next_break
            if seconds < next_break:
                return
            next_break = seconds + interval
            for client in clients:
                positions = batches[client.name]
                if not positions:
                    continue
                x, z = positions.pop(0)
                client.move(x + 0.5, 95, z + 0.5)
                events.append((client, (x, 95, z + 2), x, client.mark(), time.monotonic()))
                client.dig((x, 95, z + 1), x, face=2)

        rows.append(window(server, trackers, 'mining', args.seconds, mine))
        reveals = []
        accepted = 0
        for client, pos, sequence, mark, sent in events:
            stream = client.packets[mark:]
            seal = (pos[0], pos[1], pos[2] - 1)
            hit = next((p for p in stream if updated_state(p, seal) == 0), None)
            if hit is None:
                continue
            accepted += 1
            ack = next((p for p in stream if p.kind == ACK and Reader(p.data).varint() >= sequence), None)
            reveal = next((p for p in stream if updated_state(p, pos) == ore), None)
            if reveal is not None:
                reveals.append({
                    'client': client.name, 'x': pos[0],
                    'request_to_reveal_ms': (reveal.at - sent) * 1000,
                    'accepted_state_to_reveal_ms': (reveal.at - hit.at) * 1000,
                    'ack_to_reveal_ms': (reveal.at - ack.at) * 1000 if ack else None,
                })
        rows[-1].update({'dig_attempts': len(events), 'accepted_breaks': accepted, 'ordinary_reveals': reveals})
    return {'round': round_number, 'mode': mode, 'phases': rows}



def compare_traffic(args, output, snapshot):
    from wire import CHUNK, Client, column

    wanted = {(x, z) for x in range(-3, 4) for z in range(-3, 4)}
    rows = []
    for mode in (2, 3):
        folder = output / f'matched-mode-{mode}'
        folder.mkdir()
        shutil.copytree(snapshot, folder / 'world')
        server, port = server_for(args, folder, config_for(mode))
        with ExitStack() as stack:
            stack.callback(server.stop)
            client = Client(port, 'Traffic')
            stack.callback(client.close)
            chunks = {}
            for key in sorted(wanted):
                _, packet = client.wait(lambda p: p.kind == CHUNK and column(p) == key)
                chunks[f'{key[0]},{key[1]}'] = packet.wire_bytes
            rows.append({'mode': mode, 'bytes_by_column': chunks, 'total': sum(chunks.values()),
                         'mean': statistics.fmean(chunks.values())})
    return rows


def config_for(mode):
    return f'''
[dimensions."minecraft:overworld"]
enabled = {str(mode > 0).lower()}
mode = {max(mode, 1)}
max_block_height = 112
hidden_blocks = ["diamond_ore", "deepslate_diamond_ore", "redstone_ore", "gold_ore", "iron_ore"]
replacement_blocks = ["stone", "deepslate", "andesite", "diorite", "granite", "tuff"]
'''.lstrip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--modes', default='0,1,2,3')
    parser.add_argument('--rounds', default=4, type=int)
    parser.add_argument('--bots', default=4, type=int)
    parser.add_argument('--seconds', default=24, type=float, help='fixed duration of each measurement window')
    parser.add_argument('--speed', default=18, type=float, help='blocks per second during exploration')
    parser.add_argument('--snapshot', type=Path, help='reuse an existing saved world directory')
    parser.add_argument('--plain-binary', type=Path,
                        help='the same engine export without the mod, selected as mode -1')
    parser.add_argument('--cold-only', action='store_true',
                        help='repeat only the join and cold exploration windows')
    parser.add_argument('--traffic-only', action='store_true',
                        help='compare first compressed sends of the same 49 saved columns in modes 2 and 3')
    args = arguments(parser)
    if not 1 <= args.bots <= 12 or args.rounds < 1 or args.seconds < 6 or args.speed <= 0:
        parser.error('use 1–12 bots, positive rounds and speed, and windows of at least six seconds')
    modes = [int(s) for s in args.modes.split(',')]
    allowed_modes = {-1, 0, 1, 2, 3} if args.plain_binary else {0, 1, 2, 3}
    if not modes or not set(modes) <= allowed_modes or len(set(modes)) != len(modes):
        parser.error('modes must be unique values in 0–3, or -1 with --plain-binary')
    output = args.out / f'bench-{time.time_ns()}'
    output.mkdir()
    plain = Namespace(**{**vars(args), 'binary': args.plain_binary.resolve()}) if args.plain_binary else args
    snapshot = args.snapshot.resolve() if args.snapshot else prepare_snapshot(plain, output)
    if args.traffic_only:
        if args.view_distance < 5:
            parser.error('matched traffic requires a view distance of at least 5')
        rows = compare_traffic(args, output, snapshot)
        (output / 'traffic.json').write_text(json.dumps(rows, indent=2) + '\n')
        print(json.dumps([{k: v for k, v in row.items() if k != 'bytes_by_column'} for row in rows]))
        return
    report = {
        'machine': platform.uname()._asdict(), 'bots': args.bots, 'view_distance': args.view_distance,
        'seconds_per_phase': args.seconds, 'explore_speed': args.speed, 'snapshot': str(snapshot),
        'engine_commit': 'dd1509b82d4804a234fbc0da3263a5454948e9c4', 'trials': [],
    }
    for round_number in range(args.rounds):
        offset = round_number % len(modes)
        for mode in modes[offset:] + modes[:offset]:
            case = plain if mode == -1 else args
            row = one_trial(case, output, snapshot, mode, round_number)
            report['trials'].append(row)
            (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
            print(f"round {round_number + 1} mode {mode}: "
                  + ', '.join(f"{phase['phase']} {phase['delivered_chunks_per_second']:.1f} chunks/s"
                              for phase in row['phases']), flush=True)
    groups = defaultdict(list)
    for trial in report['trials']:
        for row in trial['phases']:
            groups[(trial['mode'], row['phase'])].append(row)
    report['summary'] = []
    for (mode, phase), rows in sorted(groups.items()):
        metrics = {}
        for field in ('p95_ms', 'p99_ms', 'delivered_chunks_per_second', 'distinct_chunks_per_second',
                      'first_time_chunks_per_second', 'compressed_framed_bytes_per_chunk',
                      'server_cpu_seconds', 'sampled_peak_server_rss_bytes'):
            values = [row[field] for row in rows if row[field] is not None]
            if values:
                metrics[field] = {'median': statistics.median(values), 'min': min(values), 'max': max(values)}
        report['summary'].append({'mode': mode, 'phase': phase, **metrics})
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
