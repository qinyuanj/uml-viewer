#!/usr/bin/env python3
"""Count routed arrows that cross a class box they do not connect.

Reads scene dumps of `uml-viewer scene --ir <ir.edn> [--focus <edn>]
--output <dump.edn>` and checks each arrow's routed points and its drawn
curve against every non-endpoint box (2px inside its edge). Arrows are
grouped by rank span: same, adjacent, long (two ranks or more) and other
(an unranked end, such as a foreign oval).

Exit 1 when a long arrow crosses a box: every rank it passes has a lane.
"""
import re
import sys
from collections import Counter

TOKEN = re.compile(r'\s*(?:(#\{)|([\[\]{}()])|("(?:\\.|[^"\\])*")|([^\s,\[\]{}()"]+))')


def parse(text):
    tokens = [m for m in TOKEN.finditer(text) if m.group(0).strip()]
    at = 0

    def value():
        nonlocal at
        m = tokens[at]
        at += 1
        if m.group(1) or m.group(2) in ('[', '('):
            close = ']' if m.group(2) == '[' else ')' if m.group(2) == '(' else '}'
            items = []
            while tokens[at].group(2) != close:
                items.append(value())
            at += 1
            return items
        if m.group(2) == '{':
            entries = {}
            while tokens[at].group(2) != '}':
                key = value()
                entries[key if isinstance(key, str) else repr(key)] = value()
            at += 1
            return entries
        if m.group(3):
            return m.group(3)[1:-1]
        atom = m.group(4)
        if atom in ('true', 'false'):
            return atom == 'true'
        if atom == 'nil':
            return None
        try:
            return float(atom)
        except ValueError:
            return atom
    return value()


def crosses(p, q, r, pad=2.0):
    x0, y0 = r[':x'] + pad, r[':y'] + pad
    x1, y1 = r[':x'] + r[':w'] - pad, r[':y'] + r[':h'] - pad
    if x1 <= x0 or y1 <= y0:
        return False
    dx, dy = q[0] - p[0], q[1] - p[1]
    t0, t1 = 0.0, 1.0
    for pp, qq in ((-dx, p[0] - x0), (dx, x1 - p[0]), (-dy, p[1] - y0), (dy, y1 - p[1])):
        if pp == 0:
            if qq < 0:
                return False
            continue
        t = qq / pp
        if pp < 0:
            t0 = max(t0, t)
        else:
            t1 = min(t1, t)
        if t0 > t1:
            return False
    return True


def curve_points(curve):
    pts, cur = [curve[':start']], curve[':start']
    for op in curve[':ops']:
        if op[':op'] == ':line':
            pts.append(op[':p'])
        else:
            a, b, c, d = cur, op[':c1'], op[':c2'], op[':p']
            for k in range(1, 13):
                t = k / 12
                u = 1 - t
                pts.append([u**3 * a[i] + 3 * u * u * t * b[i] + 3 * u * t * t * c[i] + t**3 * d[i]
                            for i in (0, 1)])
        cur = op[':p']
    return pts


def hit_boxes(pts, boxes):
    return {bid for p, q in zip(pts, pts[1:]) for bid, r in boxes if crosses(p, q, r)}


def measure(path):
    view = parse(open(path, encoding='utf-8').read())[':views'][0]
    classes = view[':classes']
    by_id = {c[':id']: c for c in classes}
    rows = []
    for e in view[':edges']:
        f, t = by_id.get(e.get(':from')), by_id.get(e.get(':to'))
        boxes = [(c[':id'], c[':rect']) for c in classes
                 if c[':id'] not in (e.get(':from'), e.get(':to')) and ':rect' in c]
        kind = 'other'
        if f and t and f.get(':rank') is not None and t.get(':rank') is not None:
            span = abs(f[':rank'] - t[':rank'])
            kind = 'same' if span == 0 else 'adjacent' if span == 1 else 'long'
        pts = e[':points']
        hits = hit_boxes(pts, boxes)
        if ':curve' in e:
            hits |= hit_boxes(curve_points(e[':curve']), boxes)
        rows.append((kind, e.get(':from'), e.get(':to'), sorted(hits)))
    return view.get(':focus'), rows


def main(paths):
    if not paths:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    failed = False
    for path in paths:
        focus, rows = measure(path)
        print(f'{path} focus={focus}')
        count, crossing = Counter(), Counter()
        for kind, f, t, hits in rows:
            count[kind] += 1
            crossing[kind] += bool(hits)
            if hits:
                print(f'  {kind}: {f} -> {t} crosses {" ".join(hits)}')
        for kind in ('same', 'adjacent', 'long', 'other'):
            if count[kind]:
                print(f'  {kind}: {count[kind]} arrows, {crossing[kind]} cross a box')
        failed |= crossing['long'] > 0
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
