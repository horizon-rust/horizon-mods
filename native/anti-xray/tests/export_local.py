#!/usr/bin/env python3
"""Export a local Linux test server from the pinned engine's prebuilt SDK output."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine', required=True, type=Path)
    parser.add_argument('--prebuilt', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    engine, prebuilt, output = (arg.resolve() for arg in (args.engine, args.prebuilt, args.output))
    sys.path.insert(0, str(engine / 'tools'))
    import prebuilt_game as sdk
    from native_dylib_probe import preflight

    if not sys.platform.startswith('linux'):
        parser.error('the local export uses Linux dylib and linker conventions')
    source = Path(__file__).resolve().parents[1]
    manifest = tomllib.loads((source / 'horizon-mod.toml').read_text())
    commit = manifest['native']['horizon']
    compiler = subprocess.check_output(['rustc', '-vV'], text=True)
    identity = preflight(prebuilt, compiler)
    if identity['commit'] != commit or identity['side'] != 'server' or sdk.head() != commit:
        parser.error(f'expected a server SDK and checkout at {commit}')
    release = next(line.split()[1] for line in compiler.splitlines() if line.startswith('release: '))
    mod, version = manifest['id'], manifest['version']
    build = f'{commit} {mod} {version} {release}'
    deps = prebuilt / 'deps'
    output.mkdir()
    library = output / f'lib{mod}.so'
    with tempfile.TemporaryDirectory(prefix='antixray-native-') as temporary:
        stage = Path(temporary) / 'src'
        shutil.copytree(source / 'src', stage)
        template = (prebuilt / sdk.NATIVE_MODS_TEMPLATE).read_text()
        rows = {
            'NATIVE_BUILD': build,
            'MOD_ENTRIES': f'"{mod}" => crate::hooks,',
            'BUILT_MODS': f'mod_api::__private::native::BuiltMod {{ id: "{mod}", version: "{version}", side: mod_api::Side::Server }},',
            'MOD_SLOTS': '', 'MOD_REDIRECTS': '',
        }
        for marker, text in rows.items():
            template = template.replace(f'/* {marker} */', text)
        (stage / 'entry.rs').write_text(template)
        with (stage / 'lib.rs').open('a') as root:
            root.write('\nmod entry;\nextern crate horizon_mod_host;\n')
        externs = [arg for crate in ('mod_api', 'horizon_mod_host', 'serde', 'toml')
                   for arg in sdk.extern_args(deps, crate)]
        subprocess.run(['rustc', '--edition', '2024', '--crate-type', 'dylib', '--crate-name', mod,
                        '-C', 'prefer-dynamic', '-C', 'opt-level=3', '-C', 'link-arg=-Wl,-rpath,$ORIGIN',
                        '-L', f'dependency={deps}', '-L', f'dependency={prebuilt / "host"}',
                        '-L', f'native={prebuilt / "native"}', *externs, '-Dwarnings', '-o', str(library),
                        str(stage / 'lib.rs')], check=True)

    installed = output / 'horizon-native-mods' / mod
    installed.mkdir(parents=True)
    for name in ('horizon-mod.toml', 'Cargo.toml', 'Cargo.lock'):
        shutil.copy2(source / name, installed / name)
    shutil.copytree(source / 'src', installed / 'src')
    for lib in sdk.runtime_libraries(prebuilt):
        shutil.copy2(lib, output / lib.name)
    shutil.copy2(prebuilt / 'bin' / sdk.EXECUTABLE, output / 'horizon-engine')
    (output / 'native-mods.json').write_text(json.dumps({'libraries': [{
        'file': library.name, 'sha256': sdk.sha256(library), 'kind': 'mod', 'mod': mod, 'identity': build,
    }]}))
    print(output / 'horizon-engine')


if __name__ == '__main__':
    main()
