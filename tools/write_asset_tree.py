"""Write the exhaustive source asset tree from a verified extraction manifest.

Documentation helper only; the runtime and extraction library do not use Python.
"""
import argparse
from collections import Counter
import json
from pathlib import Path, PurePosixPath


def build(manifest):
    digest = manifest.get('iso_sha256') or manifest.get('iso', {}).get('sha256')
    if digest != '0de05981a34156b9cedcef73c73d4244ac05cf6149ab3c9cfed917698819e464':
        raise ValueError('Expected the verified USA 1.02 manifest')
    files = manifest['files']
    if 'iso' in manifest:
        files = [row for row in files if row['path'].startswith('files/')]
        files = [{**row, 'path': row['path'][6:]} for row in files]
    paths = sorted(row['path'] for row in files)
    if len(paths) != 1209 or len(set(paths)) != len(paths):
        raise ValueError('Expected all 1,209 unique game files')
    tree = {}
    for name in paths:
        path = PurePosixPath(name)
        if path.is_absolute() or '..' in path.parts or '\\' in name:
            raise ValueError('Unsafe source path')
        node = tree
        for part in path.parts[:-1]:
            node = node.setdefault(part, {})
        node[path.name] = None
    lines = ['# Complete source asset tree', '',
             'Every game filesystem file in the supported Melee USA 1.02 image is listed',
             'below, exactly once. This is the import inventory, not a claim that the',
             'current game consumes every file or that DAT internals are already decoded.',
             'See [the canonical asset tree](asset-tree.md) for consumers and decoded categories.', '',
             f'ISO SHA-256: `{digest}`.', '',
             '| Container/extension | Files |', '| --- | ---: |']
    counts = Counter(PurePosixPath(path).suffix.lower() or '(none)' for path in paths)
    lines += [f'| `{extension}` | {count} |' for extension, count in sorted(counts.items())]
    lines += [f'| **Total** | **{len(paths)}** |', '', '```text', 'disc/files/']

    def walk(node, prefix=''):
        entries = sorted(node.items(), key=lambda pair: (pair[1] is None, pair[0]))
        for index, (name, child) in enumerate(entries):
            last = index == len(entries) - 1
            lines.append(prefix + ('└── ' if last else '├── ') + name + ('/' if child is not None else ''))
            if child is not None:
                walk(child, prefix + ('    ' if last else '│   '))
    walk(tree)
    lines += ['```', '', 'Regenerate with `uv run --no-project tools/write_asset_tree.py --manifest PATH --output docs/asset-source-tree.md`.', '']
    return '\n'.join(lines)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.write_text(build(json.loads(args.manifest.read_text())))
