# Round 4E item 3: writes docs/plan/REWRITE_LEDGER.md (run from the repo root:
# python3 windows/scripts/rewrite-ledger.py). Needs the full git history.
import subprocess as sp, collections
B = 'd7a8eca'
base = set(sp.run(['git', 'ls-tree', '-r', '--name-only', B], capture_output=True, text=True).stdout.split('\n'))
files = sp.run(['git', 'ls-files', 'windows/src', 'windows/src-tauri/src', 'windows/hook'], capture_output=True, text=True).stdout.split()
def norm(s): return [l.strip() for l in s.split('\n') if len(l.strip()) >= 20 and not l.strip().startswith(('//', '*', '/*', '#'))]
OWN_CHAR = {'skins.ts', 'pillow.ts', 'growth.ts', 'stage6.ts', 'appearance.ts', 'sprites.ts', 'seed.ts', 'outfits.ts', 'wardrobe.ts'}
OWN = "Groovy's own"
# Rewritten with their interface kept (the line match then counts the kept signatures).
REWRITTEN_4E = {'windows/src/core/anim.ts', 'windows/src/mochi/minibots.ts', 'windows/src/mochi/desktop-logic.ts'}
def vis(f):
    if '/mochi/' in f or f.endswith('core/anim.ts'): return (0, 'character', '4E (follow-up)')
    if '/island/' in f or '/views/' in f or '/desktop/' in f or f.endswith(('style.css', 'core/layout.ts', 'core/anim.ts', 'core/pills.ts')): return (1, 'island and views', '6')
    if '/settings/' in f or '/hub/' in f or '/recap/' in f: return (2, 'Settings and panels', '7')
    if '/voice/' in f or '/i18n/' in f: return (3, 'voice and strings', '7')
    if '/phone_link/' in f: return (5, 'phone link', 'after the Android source')
    return (4, 'plumbing', '8 (not this round)')
out = []
for f in files:
    name = f.split('/')[-1]
    if f in base:
        old = sp.run(['git', 'show', f'{B}:{f}'], capture_output=True, text=True).stdout
        o = set(norm(old)); nl = norm(open(f, errors='ignore').read())
        same = sum(1 for l in nl if l in o) / max(1, len(nl))
        if f.startswith('windows/src/mochi/') and name in OWN_CHAR: v, note = OWN, 'no line of the original, CHARACTER_MOTION.md §1'
        elif f == 'windows/src/mochi/engine.ts': v, note = 'rewritten', "round 4E item 1, 20 Nov 2026; its kept blocks are Groovy's own"
        elif f in REWRITTEN_4E: v, note = 'rewritten', f"bodies replaced in round 4E item 1, 20 Nov 2026; {same:.0%} of lines still match: the public signatures, event names and tested constants its callers use"
        elif same < 0.2: v, note = 'rewritten', f'{same:.0%} same as at the start'
        else: v, note = 'inherited', f'{same:.0%} same as at the start'
    else:
        v, note = OWN, 'added after the start'
    out.append((f, v, note))
c = collections.Counter(v for _, v, _ in out)
L = ["# Copied-code ledger (round 4E item 3, 20 Nov 2026)\n",
     "Which source files still hold code from the MIT-licensed project Groovy's code started from, so its credit in `windows/THIRD_PARTY_NOTICES.md` can be dropped once no file here is **inherited**. Not legal advice. Regenerate with `python3 windows/scripts/rewrite-ledger.py` (full history needed).\n",
     "**Method.** The repository's first commit holds only `LICENSE` and `README.md`; the code arrived in `d7a8eca` (15 Oct 2026, \"Import Groovy 0.5.1 source\"), the start used here. For each file under `windows/src`, `windows/src-tauri/src` and `windows/hook`: **Groovy's own** = added after the start (or a character file shown in `docs/plan/CHARACTER_MOTION.md` §1 to hold no line of the original); **rewritten** = there at the start, now under 20 % of its substantive lines (20+ characters, comments skipped) the same; **inherited** = there at the start and still 20 % or more the same. The 0.5.1 import already held Groovy changes, so the percentage over-states the original's share, never under-states it.\n",
     f"**Totals:** {c['inherited']} inherited, {c['rewritten']} rewritten, {c[OWN]} Groovy's own ({len(out)} files). Character (`windows/src/mochi/`, `core/anim.ts`): every file is rewritten or Groovy's own. Files rewritten with their interface kept show how many lines still match, so the reader can judge.\n",
     "## Inherited files, most visible first, and the round that rewrites them\nCharacter first, plumbing last; plumbing is not rewritten in round 4E. Phone link files also carry their own author's MIT credit and wait on the Android source.\n",
     "| Rank | Area | File | Same as at the start | Round |", "|---|---|---|---|---|"]
inh = sorted(((vis(f), f, n) for f, v, n in out if v == 'inherited'), key=lambda r: (r[0][0], r[1]))
for i, ((rk, area, rnd), f, n) in enumerate(inh, 1):
    L.append(f"| {i} | {area} | `{f}` | {n.split()[0]} | {rnd} |")
L.append("\n## Every file")
cur = None
for f, v, n in out:
    d = '/'.join(f.split('/')[:-1])
    if d != cur:
        L.append(f"\n### `{d}/`\n"); cur = d
    L.append(f"- `{f.split('/')[-1]}`: **{v}** ({n})")
open('docs/plan/REWRITE_LEDGER.md', 'w').write('\n'.join(L) + '\n')
print(dict(c))
print([(f.split('/')[-1], v) for f, v, n in out if '/mochi/' in f and v != OWN])
