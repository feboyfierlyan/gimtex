#!/usr/bin/env python3
"""Record real Gimtex commands in a PTY and render their output as a GIF.

Requires macOS/Linux (or WSL), Python 3, Pillow, and a built release binary.
No output is fabricated: typed commands and stdout/stderr are captured from the
shell's pseudo-terminal. Pauses are recorded to make the loop readable.
"""
import argparse
import codecs
import errno
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import time

if os.name != 'posix':
    raise SystemExit('Recording requires a POSIX pseudo-terminal; use macOS, Linux, or WSL.')

import fcntl
import pty
import termios
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
WIDTH, HEIGHT = 1280, 744
COLS, ROWS = 98, 22
STEP = 0.08
COMMANDS = ["gimtex src/ -i '*.rs' -n -o context.md", "head -n 19 context.md"]


def record(binary):
    events = []
    decoder = codecs.getincrementaldecoder('utf-8')()
    # Copy this repository's real implementation, not the small README fixture.
    # The isolated directory keeps recordings free of user paths and output files.
    with tempfile.TemporaryDirectory(prefix='gimtex-live-', dir='/tmp') as directory:
        source = Path(directory, 'src')
        shutil.copytree(ROOT / 'src', source)
        environment = dict(os.environ, PATH=str(binary.parent) + os.pathsep + os.environ.get('PATH', ''),
                           PS1='$ ', PS2='> ', TERM='dumb', NO_COLOR='1', CLICOLOR_FORCE='0')
        for key in ('ENV', 'BASH_ENV', 'PROMPT_COMMAND'):
            environment.pop(key, None)
        start = time.monotonic()
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(directory)
            os.execve('/bin/sh', ['sh', '-ei'], environment)
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', ROWS, COLS, 0, 0))
        received = ''

        def drain(seconds):
            nonlocal received
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                ready, _, _ = select.select([fd], [], [], max(0, deadline-time.monotonic()))
                if not ready:
                    break
                try:
                    chunk = os.read(fd, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        raise RuntimeError('The demo shell exited before recording finished') from error
                    raise
                if not chunk:
                    raise RuntimeError('The demo shell closed its terminal')
                text = decoder.decode(chunk)
                if text:
                    if '\x1b' in text:
                        raise RuntimeError('Unexpected ANSI escape: keep this capture in TERM=dumb with NO_COLOR=1')
                    events.append([round(time.monotonic()-start, 4), 'o', text])
                    received += text

        def prompt(timeout=30):
            deadline = time.monotonic() + timeout
            while not received.endswith('$ '):
                if time.monotonic() > deadline:
                    raise TimeoutError('Timed out waiting for the demo shell prompt')
                drain(0.05)

        try:
            prompt()
            drain(0.8)
            for index, command in enumerate(COMMANDS):
                for character in command:
                    os.write(fd, character.encode())
                    drain(0.06)
                os.write(fd, b'\n')
                # Read at least the echoed newline before testing for the next prompt.
                drain(0.1)
                prompt()
                drain(4.0 if index == 0 else 5.0)
            payload = Path(directory, 'context.md').read_text(encoding='utf-8')
            assert '# Project structure' in payload and 'main.rs' in payload and 'scanner.rs' in payload
            assert '2 files |' in received
            duration = time.monotonic()-start
        finally:
            os.kill(pid, signal.SIGTERM)
            os.close(fd)
            os.waitpid(pid, 0)
    return events, duration, payload


class Terminal:
    """The small CR/LF/backspace/tab subset emitted by this plain-text session."""
    def __init__(self):
        self.lines = [''] * ROWS
        self.x = self.y = 0

    def feed(self, text):
        for character in text:
            if character == '\r':
                self.x = 0
            elif character == '\n':
                self.newline()
            elif character == '\b':
                self.x = max(0, self.x-1)
            elif character == '\t':
                self.feed(' ' * (8-self.x % 8))
            elif character >= ' ' and character != '\x7f':
                if self.x >= COLS:
                    self.x = 0
                    self.newline()
                row = self.lines[self.y].ljust(self.x)
                self.lines[self.y] = row[:self.x] + character + row[self.x+1:]
                self.x += 1

    def newline(self):
        self.y += 1
        if self.y >= ROWS:
            self.lines.pop(0)
            self.lines.append('')
            self.y = ROWS-1


def font_path(requested):
    candidates = [requested, '/System/Library/Fonts/Menlo.ttc',
                  '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf',
                  '/usr/share/fonts/truetype/liberation2/LiberationMono-Regular.ttf']
    for path in candidates:
        if path and Path(path).is_file():
            return path
    raise SystemExit('Provide a monospace TrueType font with --font /path/to/font.ttf')


def render(events, duration, font):
    normal = ImageFont.truetype(font, 20)
    small = ImageFont.truetype(font, 14)
    label = ImageFont.truetype(font, 17)
    frames = []
    terminal = Terminal()
    event_index = 0
    total_frames = int(duration / STEP) + 1
    for index in range(total_frames):
        position = index * STEP
        while event_index < len(events) and events[event_index][0] <= position:
            terminal.feed(events[event_index][2])
            event_index += 1
        frame = Image.new('RGB', (WIDTH, HEIGHT), '#09140e')
        draw = ImageDraw.Draw(frame)
        draw.rounded_rectangle((1, 1, WIDTH-2, HEIGHT-2), radius=20, outline='#31513b', width=2)
        draw.line((1, 69, WIDTH-2, 69), fill='#294432')
        draw.ellipse((31, 29, 41, 39), fill='#aaf59c')
        draw.text((57, 22), 'GIMTEX / REAL CLI SESSION', font=label, fill='#e4f1e7')
        draw.text((938, 26), 'RUST SOURCE -> CONTEXT', font=small, fill='#8eaf98')
        for row, line in enumerate(terminal.lines):
            color = '#def0e3'
            if line.startswith('$ ') or line.startswith('[i]') or line.startswith('## '):
                color = '#aaf59c'
            elif line.startswith('[!]'):
                color = '#e8cd8b'
            elif line.startswith('[>>]') or line.startswith('```'):
                color = '#8eaf98'
            draw.text((37, 90 + row*26), line, font=normal, fill=color)
        if int(position * 2) % 2 == 0 and terminal.x < COLS:
            x = 37 + normal.getlength('M') * terminal.x
            y = 90 + terminal.y * 26
            draw.rectangle((int(x), y+3, int(x)+10, y+22), fill='#aaf59c')
        draw.line((1, 686, WIDTH-2, 686), fill='#294432')
        draw.text((37, 703), "Recorded on Gimtex's own src/  |  export, then inspect  |  loops", font=small, fill='#9ab5a3')
        draw.line((21, HEIGHT-9, 21 + int((WIDTH-42)*index/max(1,total_frames-1)), HEIGHT-9), fill='#7fb884', width=2)
        frames.append(frame)
    # Use a real completed-export frame as a useful poster for nonanimated viewers.
    metric_time = next(event[0] for event in events if 'Payload Metrics:' in event[2])
    poster_index = min(len(frames)-1, int(metric_time / STEP) + 2)
    frames[poster_index].save(ROOT / 'assets/readme/hero-poster.png')
    frames.insert(0, frames[poster_index].copy())
    # One palette avoids flicker between frames; delta-frame encoding keeps it small.
    palette = frames[-1].quantize(colors=48)
    encoded = [frame.quantize(palette=palette, dither=Image.Dither.NONE) for frame in frames]
    target = ROOT / 'assets/readme/hero.gif'
    encoded[0].save(target, save_all=True, append_images=encoded[1:], duration=[1200] + [int(STEP*1000)] * (len(encoded)-1),
                    loop=0, optimize=True, disposal=1)
    return target


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--font', help='Path to a monospace TrueType/OpenType font')
    args = parser.parse_args()
    binary = ROOT / 'target/release/gimtex'
    if not binary.exists():
        raise SystemExit('Build first: cargo build --locked --release')
    font = font_path(args.font)
    events, duration, payload = record(binary)
    media = ROOT / 'docs/media'
    cast = {'version': 2, 'width': COLS, 'height': ROWS,
            'title': 'Gimtex exporting its own Rust implementation',
            'env': {'TERM': 'dumb', 'SHELL': '/bin/sh'}}
    with (media / 'hero.cast').open('w', encoding='utf-8') as stream:
        for item in [cast, *events]:
            stream.write(json.dumps(item, ensure_ascii=False) + '\n')
    (media / 'hero-transcript.txt').write_text('\n'.join(line.rstrip() for line in ''.join(event[2] for event in events).splitlines()) + '\n', encoding='utf-8')
    metadata = {'commands': COMMANDS, 'duration_seconds': round(duration, 3),
                'binary_version': subprocess.check_output([str(binary), '--version'], text=True).strip(),
                'inputs': {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
                           for path in sorted((ROOT / 'src').rglob('*.rs'))},
                'payload_sha256': hashlib.sha256(payload.encode()).hexdigest(),
                'notes': 'Actual PTY output. Source copied unchanged to a temporary workspace. GIF opens with a 1.2-second still of the completed export, then replays the recording. Typing and reading pauses are intentional; this is not a benchmark.'}
    (media / 'hero-recording.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    target = render(events, duration, font)
    print(f'Recorded {len(events)} output events over {duration:.1f}s; GIF: {target.stat().st_size:,} bytes')
    print('Commands and their output were captured from a real shell PTY; no output was substituted.')


if __name__ == '__main__':
    main()
