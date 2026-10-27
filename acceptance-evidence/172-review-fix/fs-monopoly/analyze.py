"""fsm_analyze.py projection.edn -- bundle/branch statistics for a `uml-viewer scene` projection."""
import math, re, sys

TOKEN = re.compile(r'\s*(?:([\[\]{}()])|("(?:[^"\\]|\\.)*")|([^\s\[\]{}(),"]+)|,)')

def parse(text):
    pos = 0
    def atom(t):
        if t.startswith(':'): return t
        if t in ('true', 'false'): return t == 'true'
        if t == 'nil': return None
        try: return float(t)
        except ValueError: return t
    def value():
        nonlocal pos
        m = TOKEN.match(text, pos)
        while m and m.group(0).strip() in (',', ''):
            if m.end() == pos: raise ValueError(pos)
            pos = m.end(); m = TOKEN.match(text, pos)
        pos = m.end()
        br, st, at = m.groups()
        if st: return st[1:-1]
        if at: return atom(at)
        close = {'[': ']', '{': '}', '(': ')'}[br]
        items = []
        while True:
            m = TOKEN.match(text, pos)
            if m.group(1) == close:
                pos = m.end(); break
            items.append(value())
        return dict(zip(items[::2], items[1::2])) if br == '{' else items
    return value()

def length(ps):
    return sum(math.dist(a, b) for a, b in zip(ps, ps[1:]))

def turns(ps):
    """Largest heading change between segments 3+ px long, in degrees."""
    pts = [ps[0]]
    for p in ps[1:]:
        if math.dist(pts[-1], p) >= 3: pts.append(p)
    worst = 0.0
    for a, b, c in zip(pts, pts[1:], pts[2:]):
        h1 = math.atan2(b[1]-a[1], b[0]-a[0]); h2 = math.atan2(c[1]-b[1], c[0]-b[0])
        d = abs((h2 - h1 + math.pi) % (2*math.pi) - math.pi)
        worst = max(worst, math.degrees(d))
    return worst

doc = parse(open(sys.argv[1]).read())
view = doc[':views'][0] if ':views' in doc else doc[':scene']
edges, bundles = view[':edges'], view.get(':bundles') or []
sizes = sorted(len(b[':members']) for b in bundles)
print(f'edges {len(edges)} bundles {len(bundles)} member sizes {sizes}')
trunk_lengths = sorted(round(length(b[':strokes'][0])) for b in bundles)
print('trunk lengths', trunk_lengths)
branched = [e for e in edges if e.get(':branches')]
pieces = [p for e in branched for p in e[':branches']]
print(f'edges with branches {len(branched)}, branch pieces {len(pieces)}')
short = sorted(round(length(p), 1) for p in pieces if length(p) < 24)
print('branch pieces shorter than 24px:', short)
bad_ends = 0
for e in branched:
    s = e[':strokes']; b = e[':branches']
    if math.dist(s[0][0], b[0][0]) > 1e-6 or math.dist(s[-1][-1], b[-1][-1]) > 1e-6:
        bad_ends += 1
print('edges whose branches do not start/end at the base stroke ends:', bad_ends)
base_short = sorted(round(length(p), 1) for e in edges for p in (e.get(':strokes') or []) if length(p) < 24)
print('base stroke pieces shorter than 24px:', base_short)
worst = sorted((round(turns(p)), round(turns(s))) for e in branched for p, s in [(max(e[':branches'], key=lambda q: turns(q)), max(e[':strokes'], key=lambda q: turns(q)))])
print('sharpest branch turn vs sharpest base turn (deg), top 6:', sorted(worst, reverse=True)[:6])
