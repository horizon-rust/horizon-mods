#!/usr/bin/env python3
"""Join the optimized export with an unmodified Minecraft Java 26.3 client under Xvfb."""

import argparse
from argparse import Namespace
from contextlib import ExitStack
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading

from dedicated import SCENE, arguments, server_for


def run(args, folder):
    import lab
    import vanilla_client

    if not os.environ.get('DISPLAY') or os.environ.get('WAYLAND_DISPLAY'):
        raise RuntimeError('run under the pinned checkout’s tools/ci/with_xvfb.py')
    server_dir, client_dir = folder / 'server', folder / 'client'
    server_dir.mkdir()
    client_dir.mkdir()
    # Read inputs from lab.py prepare output. The Java client writes only to client_dir.
    vanilla_client.ROOT = args.vanilla_root.resolve()
    vanilla_client.GAME = vanilla_client.ROOT / '.local' / 'minecraft' / lab.VERSION
    assets = client_dir / 'assets'
    (assets / 'indexes').mkdir(parents=True)
    for index in (args.assets / 'indexes').glob('*.json'):
        shutil.copy2(index, assets / 'indexes' / index.name)
    (assets / 'objects').symlink_to((args.assets / 'objects').resolve(), target_is_directory=True)

    server, port = server_for(args, server_dir, SCENE, flat=True)
    with ExitStack() as stack:
        stack.callback(server.stop)
        server.console('gamerule spawn_mobs false')
        server.console('setworldspawn 0 -60 0')
        server.console('gamerule respawn_radius 0')
        server.console('fill 2 -59 2 6 -55 6 minecraft:stone')
        server.console('setblock 4 -57 4 minecraft:redstone_ore')

        command_args = Namespace(game_dir=client_dir, assets=assets, heap='2G', name='ActualVanilla',
                                 width=640, height=480, join=f'127.0.0.1:{port}', world=None,
                                 lan_port=None, backend='opengl')
        vanilla_client.write_options(client_dir, {})
        env = vanilla_client.environment()
        for key in ('HORIZON_HOME', 'HORIZON_LAUNCHER_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME',
                    'HORIZON_LAUNCHER_TRASH', 'HORIZON_LAUNCHER_CLIPBOARD', 'HORIZON_LAUNCHER_DESKTOP'):
            env[key] = tempfile.mkdtemp(dir=client_dir, prefix=key.lower() + '-')
        env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', LIBGL_ALWAYS_SOFTWARE='1', SDL_VIDEODRIVER='x11')
        with (client_dir / 'console.log').open('w') as log:
            client = subprocess.Popen(vanilla_client.command(command_args), cwd=client_dir, env=env,
                                      stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                                      encoding='utf-8', errors='replace')
            rendered = threading.Event()

            def record():
                for line in client.stdout:
                    log.write(line)
                    log.flush()
                    if 'Chunk Sections Instanced' in line:
                        rendered.set()

            reader = threading.Thread(target=record, daemon=True)
            reader.start()
            try:
                server.wait_for('ActualVanilla joined the game', 90)
                assert rendered.wait(30), 'the stock client did not report rendering terrain'
                assert client.poll() is None, 'the stock client exited'
                assert not any('ActualVanilla lost connection' in line for line in server.since(0))
            finally:
                client.terminate()
                client.wait(timeout=15)
                reader.join(timeout=5)
    print('Unmodified Minecraft Java 26.3 joined and rendered terrain.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--vanilla-root', type=Path, required=True,
                        help='checkout whose .local has the original client and libraries imported by lab.py')
    parser.add_argument('--assets', type=Path, required=True, help='assets directory belonging to that client')
    args = arguments(parser)
    folder = Path(tempfile.mkdtemp(dir=args.out, prefix='vanilla-client-'))
    run(args, folder)


if __name__ == '__main__':
    main()
