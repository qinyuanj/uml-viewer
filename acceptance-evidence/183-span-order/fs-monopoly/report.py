"""#183 前后对照：<带 PIL 的 python> -I report.py。读 before/ 与 after/ 的画布 dump，
断言类集合、rank、框尺寸与关系语义不变，写 metrics.json 与 compare-{root,ui}.png。"""
from pathlib import Path
import sys, runpy, contextlib, io, json, math
from PIL import Image, ImageDraw
run = Path(__file__).resolve().parent
analyze = run.parent.parent / '172-review-fix/fs-monopoly/analyze.py'
sys.argv = ['analyze', str(run / 'before/root-canvas.edn')]
with contextlib.redirect_stdout(io.StringIO()):
    helpers = runpy.run_path(str(analyze))
parse, length = helpers['parse'], helpers['length']
variants = ['before', 'after']

def thin(ps, step=4.0):
    out = [ps[0]]
    for p in ps[1:]:
        if math.dist(out[-1], p) >= step: out.append(p)
    if out[-1] != ps[-1]: out.append(ps[-1])
    return out

def cross(a, b, c, d):
    def o(p, q, r): return (q[0]-p[0])*(r[1]-p[1]) - (q[1]-p[1])*(r[0]-p[0])
    return o(a, b, c)*o(a, b, d) < 0 and o(c, d, a)*o(c, d, b) < 0

def crossings(objects):
    """不同绘制对象（每条边自身的线段、每条共享主干）之间的真交叉数。"""
    segs = []
    for k, polys in enumerate(objects):
        for ps in polys:
            ps = thin(ps)
            for a, b in zip(ps, ps[1:]):
                segs.append((k, a, b, min(a[0], b[0]), max(a[0], b[0]), min(a[1], b[1]), max(a[1], b[1])))
    segs.sort(key=lambda s: s[3])
    n = 0
    for i, s in enumerate(segs):
        for t in segs[i+1:]:
            if t[3] > s[4]: break
            if s[0] == t[0] or t[6] < s[5] or t[5] > s[6]: continue
            if cross(s[1], s[2], t[1], t[2]): n += 1
    return n

def R(c):
    r = c[':rect']
    return (r[':x'], r[':y'], r[':w'], r[':h']) if isinstance(r, dict) else tuple(r)

rows = []
for view in ['root', 'ui']:
    base = parse((run / f'before/{view}-canvas.edn').read_text())[':scene']
    shape = lambda sc: sorted((c[':id'], c.get(':rank'), R(c)[2], R(c)[3]) for c in sc[':classes'])
    semantic = lambda es: [[e.get(k) for k in [':from', ':to', ':kind', ':deps', ':head']] for e in es]
    images = []
    for variant in variants:
        scene = parse((run / f'{variant}/{view}-canvas.edn').read_text())[':scene']
        # 类集合、rank、框尺寸与关系语义必须不变；只有横向顺序和位置可以变。
        assert shape(scene) == shape(base) and semantic(scene[':edges']) == semantic(base[':edges'])
        edges, bundles = scene[':edges'], scene.get(':bundles') or []
        cx = {c[':id']: R(c)[0] + R(c)[2]/2 for c in scene[':classes']}
        pairs = [(e[':from'], e[':to']) for e in edges if e[':from'] in cx and e[':to'] in cx]
        span = sum(abs(cx[f]-cx[t]) for f, t in pairs)
        env = {}
        for f, t in pairs:
            lo, hi = env.get(t, (cx[t], cx[t])); env[t] = (min(lo, cx[f]), max(hi, cx[f]))
        fanin = sum(hi-lo for lo, hi in env.values())
        drawn = [e.get(':branches') or e.get(':strokes') or [] for e in edges]
        trunks = [b[':strokes'] for b in bundles]
        tips = [math.dist(a[':tip'], b[':tip']) for i, a in enumerate(edges) for b in edges[i+1:] if a[':to'] == b[':to']]
        order = [c[':id'] for c in sorted(scene[':classes'], key=lambda c: (c.get(':rank') or 0, R(c)[0]))]
        view_record = (run / f'{variant}/{view}-view.edn').read_text()
        rows.append(dict(view=view, variant=variant, bundles=len(bundles),
                         trunk_length=round(sum(length(p) for t in trunks for p in t)),
                         drawn_length=round(sum(length(p) for ps in drawn + trunks for p in ps)),
                         crossings=crossings(drawn + trunks), dependency_span=round(span), fanin_envelope=round(fanin),
                         min_same_target_tip_distance=round(min(tips), 3) if tips else None,
                         scene_width=round(scene[':size'][':w']), order=order, view_record=view_record))
        with (run / variant / f'{view}-analysis.txt').open('w') as output, contextlib.redirect_stdout(output):
            sys.argv = ['analyze', str(run / variant / f'{view}-canvas.edn')]
            runpy.run_path(str(analyze))
        im = Image.open(run / variant / f'{view}.png').convert('RGB')
        mask = Image.new('1', im.size); pix = im.load(); m = mask.load()
        for y in range(im.height):
            for x in range(im.width - 300):
                r, g, b = pix[x, y]
                if (r > 150 and 65 < g < 190 and b < 110 and r > g*1.18) or (y > 100 and g > 95 and g > r*1.05 and b < g and r > 55):
                    m[x, y] = 1
        images.append((variant, im, mask.getbbox()))
    boxes = [b for _, _, b in images]
    crop = (max(0, min(b[0] for b in boxes)-18), max(0, min(b[1] for b in boxes)-12),
            max(b[2] for b in boxes)+18, max(b[3] for b in boxes)+18)
    panels = []
    for variant, im, _ in images:
        im = im.crop(crop)
        panel = Image.new('RGB', (im.width, im.height+28), '#20252a')
        panel.paste(im, (0, 28)); ImageDraw.Draw(panel).text((8, 8), variant, fill='white')
        panels.append(panel)
    across = view == 'root'
    combined = Image.new('RGB', (sum(p.width for p in panels), max(p.height for p in panels)) if across
                         else (max(p.width for p in panels), sum(p.height for p in panels)))
    at = 0
    for p in panels:
        combined.paste(p, (at, 0) if across else (0, at)); at += p.width if across else p.height
    combined.save(run / f'compare-{view}.png')
(run / 'metrics.json').write_text(json.dumps(rows, ensure_ascii=False, indent=2) + '\n')
for r in rows:
    print({k: v for k, v in r.items() if k != 'view_record'})
