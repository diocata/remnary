#!/usr/bin/env python3
"""Rebuild original, self-contained SVG documentation artwork (standard library only)."""
from pathlib import Path
from html import escape

OUT = Path(__file__).resolve().parent.parent / 'docs/assets'
INK, BLUE, TEAL, MUTED, PAPER, LINE = '#1b263b', '#4e5bd9', '#279caf', '#64728a', '#f2f5fa', '#dce3ed'


def text(x, y, value, size=20, color=INK, weight=400, family='Arial, sans-serif'):
    return f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" font-weight="{weight}" fill="{color}">{escape(value)}</text>'


def write(name, width, height, title, description, body):
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / name).write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc"><title id="title">{escape(title)}</title><desc id="desc">{escape(description)}</desc><rect width="{width}" height="{height}" rx="24" fill="{PAPER}"/>{body}</svg>\n')


def main():
    body = text(58, 69, 'A LIVING RECORD, BUILT FROM THE TRACES YOU LEAVE', 15, MUTED)
    body += '<g fill="none" stroke="#279caf" stroke-width="2.5"><ellipse cx="82" cy="139" rx="29" ry="14" transform="rotate(-40 82 139)"/><ellipse cx="82" cy="139" rx="29" ry="14" transform="rotate(40 82 139)"/></g><circle cx="82" cy="139" r="5" fill="#4e5bd9"/>'
    body += text(128, 163, 'remnary.', 72, INK, 700)
    body += text(58, 249, 'Your thoughts leave traces.', 35, INK, 600)
    body += text(58, 296, 'We connect them.', 35, BLUE, 600)
    body += text(58, 360, 'Ideas. Decisions. Preferences. Original words.', 20, MUTED)
    body += text(58, 450, 'RUST   /   ENCRYPTED SQLITE   /   READABLE MARKDOWN', 15, MUTED)
    # The signature motif: fragmented quotations stitched into one journal.
    for x,y,phrase in [(754,86,'“I prefer…”'),(719,210,'“I chose…”'),(765,334,'“Keep it simple.”')]:
        body += f'<rect x="{x}" y="{y}" width="225" height="84" rx="12" fill="white" stroke="{LINE}"/>'
        body += text(x+20,y+36,phrase,21,INK,500)
        body += f'<path d="M{x+20} {y+57} h85 m10 0 h38" stroke="{LINE}" stroke-width="5" stroke-linecap="round"/>'
        body += f'<path d="M{x+225} {y+42} C1082 {y+42} 1001 251 1060 251" fill="none" stroke="{TEAL}" stroke-width="2" stroke-dasharray="4 5"/>'
    body += f'<rect x="1060" y="184" width="160" height="145" rx="12" fill="{INK}"/><path d="M1082 184 v145" stroke="{TEAL}" stroke-width="3"/>'
    body += text(1097,225,'a living',19,'#ffffff',600)+text(1097,253,'record',24,'#ffffff',600)
    body += '<path d="M1097 281 h90 m-90 14 h64" stroke="#7a8aa8" stroke-width="3" stroke-linecap="round"/>'
    write('hero.svg',1280,500,'Remnary — Your thoughts leave traces. We connect them.','Original illustration: fragments of conversation connect to a personal journal.',body)

    body=text(40,57,'FROM FRAGMENTS TO A RECORD YOU CAN CHECK',23,INK,700)
    steps=[('Select sources','Exports + local logs'),('Keep evidence','Encrypted archive'),('Find connections','Rules / local model'),('Review the claim','Confirm or correct'),('Read your record','Dashboard + vault')]
    for n,(title,detail) in enumerate(steps):
        x=40+n*246
        body+=f'<rect x="{x}" y="104" width="220" height="145" rx="14" fill="white" stroke="{LINE}"/>'
        body+=text(x+18,139,str(n+1).zfill(2),17,TEAL,600)+text(x+18,181,title,20,INK,600)+text(x+18,216,detail,16,MUTED)
        if n<4:body+=f'<path d="M{x+224} 177 h17 l-5 -5 m5 5 l-5 5" fill="none" stroke="{BLUE}" stroke-width="2"/>'
    body+=text(40,295,'Exact source links stay attached. Recurrence never confirms a personal fact.',18,MUTED)
    write('workflow.svg',1280,330,'The Remnary evidence workflow','Select sources, retain encrypted evidence, extract candidate connections, review, and read the dashboard or Markdown vault.',body)


if __name__ == '__main__':
    main()
