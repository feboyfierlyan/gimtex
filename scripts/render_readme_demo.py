#!/usr/bin/env python3
"""Capture a real Gimtex export and render a dependency-free SVG transcript."""
import html
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target' / 'release' / ('gimtex.exe' if os.name == 'nt' else 'gimtex')
SOURCE = ROOT / 'examples' / 'readme-demo' / 'src'
ASSETS = ROOT / 'assets' / 'readme'
MEDIA = ROOT / 'docs' / 'media'
COMMAND = "gimtex examples/readme-demo/src -i '*.rs' -n -o context.md"


def text(x, y, value, size=18, fill='#dbe8df', family='monospace', weight=400):
    return (f'<text x="{x}" y="{y}" font-size="{size}" fill="{fill}" '
            f'font-family="{family}" font-weight="{weight}" '
            f'xml:space="preserve">{html.escape(value)}</text>')


def main():
    if not BINARY.exists():
        raise SystemExit('Build first: cargo build --locked --release')
    environment = dict(os.environ, NO_COLOR='1', CLICOLOR_FORCE='0')
    with tempfile.TemporaryDirectory(prefix='gimtex-docs-') as directory:
        result = subprocess.run(
            [str(BINARY), str(SOURCE), '-i', '*.rs', '-n', '-o', 'context.md'],
            cwd=directory, env=environment, capture_output=True, text=True, check=True,
        )
        payload = Path(directory, 'context.md').read_text(encoding='utf-8')
    diagnostic = result.stderr.replace(str(SOURCE.resolve()), 'examples/readme-demo/src')
    # Windows canonical paths may use the extended-length path prefix.
    diagnostic = diagnostic.replace('\\\\?\\', '')
    diagnostic = diagnostic.replace(str(SOURCE.resolve()), 'examples/readme-demo/src')
    transcript = '$ ' + COMMAND + '\n' + diagnostic
    ASSETS.mkdir(parents=True, exist_ok=True)
    MEDIA.mkdir(parents=True, exist_ok=True)
    (MEDIA / 'demo-transcript.txt').write_text(transcript, encoding='utf-8')
    (MEDIA / 'demo-context.md').write_text(payload, encoding='utf-8')
    lines = payload.splitlines()
    height = max(820, 260 + 23 * len(lines))
    parts = [f'''<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="{height}" viewBox="0 0 1280 {height}" role="img" aria-labelledby="title desc">
<title id="title">Gimtex: a recorded export and its actual Markdown output</title>
<desc id="desc">Two Rust source files are exported with line numbers. The left panel shows the command and its diagnostics; the right panel shows the complete generated context.md. The absolute scan path is shortened for readability.</desc>
<rect width="1280" height="{height}" rx="22" fill="#0a1410"/>
<rect x="1" y="1" width="1278" height="{height-2}" rx="21" fill="none" stroke="#2b4033"/>
<path d="M0 66H1280 M600 66V{height-70}" stroke="#2b4033"/>
<circle cx="32" cy="33" r="5" fill="#91b299"/><circle cx="50" cy="33" r="5" fill="#607e68"/><circle cx="68" cy="33" r="5" fill="#3e5a47"/>
''', text(92, 39, 'gimtex / recorded session', 16, '#b9cdc0'),
        text(1238, 39, '01', 15, '#aaf5a1')]
    parts += [text(36, 119, 'RUN THE COMMAND', 13, '#aaf5a1', weight=700),
              text(36, 163, '$ gimtex examples/readme-demo/src', 18, '#f0f5f1'),
              text(58, 192, "-i '*.rs' -n -o context.md", 18, '#aaf5a1')]
    y = 254
    for line in diagnostic.splitlines():
        if line.startswith('[>>]'):
            parts += [text(36, y, '[>>] Scanning target:', 16, '#99afa1'),
                      text(36, y+26, '     examples/readme-demo/src', 16, '#99afa1')]
            y += 64
        else:
            # Wrap long metrics at the separators, without changing their values.
            if line.startswith('[i] Payload Metrics:'):
                parts.append(text(36, y, '[i] Payload Metrics:', 16, '#99afa1'))
                y += 28
                parts.append(text(36, y, line.split(': ', 1)[1], 17, '#aaf5a1'))
            else:
                parts.append(text(36, y, line, 16, '#dbe8df'))
            y += 48
    parts += [f'<rect x="36" y="{height-266}" width="524" height="144" rx="12" fill="#12231a" stroke="#294533"/>',
              text(58, height-232, 'ONE FILE. THE CONTEXT YOU SELECTED.', 13, '#aaf5a1', weight=700),
              text(58, height-200, 'A readable tree, numbered source,', 18, '#dbe8df', 'sans-serif'),
              text(58, height-174, 'and token counts in a single export.', 18, '#dbe8df', 'sans-serif'),
              text(634, 119, 'context.md', 17, '#aaf5a1', weight=700)]
    for i, line in enumerate(lines):
        color = '#aaf5a1' if line.startswith('#') else '#708e79' if line.startswith('```') else '#dbe8df'
        parts.append(text(634, 162 + i*23, line, 16, color))
    parts += [f'<path d="M0 {height-70}H1280" stroke="#2b4033"/>',
              text(36, height-28, 'Actual CLI output · synthetic two-file fixture · path shortened · not a benchmark', 14, '#99afa1', 'sans-serif'), '</svg>']
    (ASSETS / 'terminal.svg').write_text('\n'.join(parts), encoding='utf-8')
    print(transcript, end='')
    print(f'Wrote transcript, payload and SVG ({len(lines)} payload lines).')


if __name__ == '__main__':
    main()
