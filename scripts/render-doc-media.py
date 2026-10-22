#!/usr/bin/env python3
"""Render guided GIFs from fictional demo captures and actual synthetic CLI output.

Optional documentation dependency: Pillow. No real histories or account services
are read. Dashboard screenshots must be captured separately from scripts/demo.py.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import textwrap

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'docs/assets'


def font(size, mono=False):
    candidates = ([Path('/System/Library/Fonts/Menlo.ttc'),
                   Path('/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf')]
                  if mono else [Path('/System/Library/Fonts/Supplemental/Arial.ttf'),
                                Path('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf')])
    for path in candidates:
        if path.exists():
            return ImageFont.truetype(str(path), size)
    return ImageFont.load_default(size=size)


def save_gif(frames, name, durations):
    # A shared palette keeps static UI text/colors stable between captured steps.
    sheet = Image.new('RGB', (frames[0].width, sum(im.height for im in frames)))
    y = 0
    for im in frames:
        sheet.paste(im, (0, y))
        y += im.height
    palette = sheet.quantize(colors=256)
    converted = [im.quantize(palette=palette, dither=Image.Dither.NONE) for im in frames]
    converted[0].save(OUT / name, save_all=True, append_images=converted[1:],
                      duration=durations, loop=0, optimize=True, disposal=2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', default='remnary')
    parser.add_argument('--screenshots', type=Path,
                        help='directory containing synthetic demo-01.png through demo-06.png')
    args = parser.parse_args()
    binary = shutil.which(args.binary)
    if binary is None:
        parser.error('provide an installed or built Remnary binary')
    OUT.mkdir(parents=True, exist_ok=True)
    if args.screenshots:
        labels = ['Review a candidate with recurring evidence', 'Open the exact source revision',
                  'Confirm the fictional preference', 'See only reviewed knowledge in your profile',
                  'Search the original words', 'Distinguish consideration from a decision']
        frames = []
        for number, label in enumerate(labels, 1):
            capture = Image.open(args.screenshots / f'demo-{number:02}.png').convert('RGB')
            frame = Image.new('RGB', (capture.width, capture.height + 78), '#1b263b')
            frame.paste(capture, (0, 0))
            draw = ImageDraw.Draw(frame)
            draw.text((20, capture.height + 12), f'{number:02} / {label}', font=font(19), fill='white')
            draw.text((20, capture.height + 44), 'Captured steps · fictional history · no model required',
                      font=font(14), fill='#a9bdce')
            frames.append(frame)
        save_gif(frames, 'dashboard-demo.gif', [2600, 3300, 2300, 2900, 2500, 2900])

    with tempfile.TemporaryDirectory(prefix='remnary-doc-cli-') as temporary:
        work = Path(temporary)
        key, archive = work / 'recovery.key', work / 'archive'
        subprocess.run([binary, 'key', 'create', str(key)], check=True, capture_output=True)
        command = [binary, '--data-dir', str(archive), '--key-file', str(key)]
        stages = []

        def capture(label, *arguments):
            result = subprocess.run([*command, *arguments], check=True, capture_output=True, text=True)
            stages.append((label, result.stdout.replace(str(work), '<demo>')))
            return result.stdout

        capture('remnary init', 'init')
        capture('remnary import examples/demo/conversations.json', 'import', str(ROOT / 'examples/demo/conversations.json'))
        capture('remnary analyze', 'analyze')
        capture('remnary discover --limit 2', 'discover', '--limit', '2')
        findings = json.loads(subprocess.check_output([*command, 'discover', '--json']))
        preference = next(x for x in findings if 'prefer Rust for local tools' in x['observation']['summary'])
        capture('remnary feedback <preference-id> confirm', 'feedback', preference['id'], 'confirm')
        capture('remnary profile', 'profile')
        capture('remnary vault sync <demo>/vault', 'vault', 'sync', str(work / 'vault'))
        frames = []
        transcript = []
        for number, (label, result) in enumerate(stages, 1):
            frame = Image.new('RGB', (1000, 570), '#172237')
            d = ImageDraw.Draw(frame)
            d.rounded_rectangle((16, 16, 984, 554), radius=16, fill='#1e2c44', outline='#40516a')
            for x, color in [(39, '#e4a2a6'), (59, '#edce91'), (79, '#7fc4b4')]:
                d.ellipse((x, 34, x+10, 44), fill=color)
            d.text((110, 29), 'remnary / fictional archive / actual CLI output', font=font(17), fill='#b6c9da')
            d.text((38, 85), '$ ' + label, font=font(19, True), fill='#9ad6df')
            lines = []
            for line in result.splitlines():
                lines.extend(textwrap.wrap(line, width=87, replace_whitespace=False) or [''])
            for y, line in enumerate(lines[:15]):
                d.text((38, 128+y*23), line, font=font(17, True), fill='#e5eaf3')
            d.text((38, 508), f'{number:02}/{len(stages):02}   Captured output excerpts · IDs abbreviated in commands', font=font(15), fill='#a9bdce')
            frames.append(frame)
            transcript.append('$ ' + label + '\n' + result)
        save_gif(frames, 'cli-demo.gif', [1700, 2200, 2200, 3400, 2500, 3000, 2400])
        # Fixed replacement means no temporary path, key or session token is published.
        (OUT / 'cli-transcript.txt').write_text('\n'.join(transcript))
    print('Documentation GIFs rendered from synthetic data.')


if __name__ == '__main__':
    main()
