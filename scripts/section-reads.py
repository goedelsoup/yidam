#!/usr/bin/env python3
"""Measure whether an agent reads a prelude section or the whole file (#967).

Run by hand, not in CI. Two subcommands:

    $ python3 scripts/section-reads.py census [LOG_DIR ...]
    $ python3 scripts/section-reads.py trial RUNS_DIR

`census` reads Claude Code session logs (default: every directory under
~/.claude/projects except this repository's own) and classifies every read of a file under
a `prelude/` directory. The `Read` tool is the minority channel — most prelude reading goes
through Bash — so shell reads are classified too:

    Read              whole, or ranged by offset/limit
    cat               whole
    sed -n 'a,bp'     ranged; head -N is ranged from line 1
    grep -A/-B/-C     excerpt (a window bounded by a pattern, not by a heading)
    awk '/a/,/b/'     excerpt
    grep, rg          search, not a read

A ranged read is then resolved against the file as it stood in its repository's git history
at the session's timestamp. A read from before genesis — when the repository was still a
copy of the template — is resolved against this repository instead. Tool calls are
de-duplicated by id, because a resumed session replays its history into a new log.

Ranged reads are sorted into: paged (abutting ranges in one session — an agent `cat`s a
file, hits the Bash output cap, and pages through it; that is a whole-file read), covers
(the range is the file), head (starts at line 1), and mid-file. A mid-file read "starts at a
heading" when a heading sits within two lines of its first line, and is "one section" when
it also ends within three lines of the next heading at that level or above.

`trial` reads the transcripts `section-reads-trial.sh` leaves in RUNS_DIR and prints, per
run, every read of a prelude file, with each read of GRAPH.md classified against the
section the arms route to.
"""
import collections
import glob
import json
import os
import re
import shlex
import subprocess
import sys
from functools import lru_cache

TEMPLATE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PRELUDE = re.compile(r'(?:^|/)prelude/([A-Za-z0-9_./-]+\.md)$')
NUMBERED = re.compile(r'^\s*(\d+)[\t→](.*)$')
HEADING = re.compile(r'^(#{1,6})\s')
OPS = {'|', '||', '&&', ';', '&', ';;', '|&'}


def rel(path):
    m = PRELUDE.search(path or '')
    return m.group(1) if m else None


# -- reading a Bash command ------------------------------------------------------------------

def segments(command):
    """Split a command line into simple commands. Quoted `\\|` in a grep pattern stays put."""
    if re.search(r"<<-?\s*'?EOF", command):
        command = command.split('<<', 1)[0]
    lex = shlex.shlex(command.replace('\n', ' ; '), posix=True, punctuation_chars=';&|')
    lex.whitespace_split = True
    seg = []
    try:
        for t in lex:
            if t in OPS:
                yield seg
                seg = []
            else:
                seg.append(t)
    except ValueError:
        pass
    yield seg


def classify_command(tokens):
    """-> [(kind, path, (start, end) or None)] for the prelude files one command reads."""
    if not tokens:
        return []
    cmd, args = os.path.basename(tokens[0]), tokens[1:]
    files = [t for t in args if rel(t)]
    if not files:
        return []
    if cmd == 'cat':
        return [('whole', f, None) for f in files]
    if cmd == 'sed':
        for a in args:
            m = re.match(r'^(\d+)(?:,(\d+|\$))?p$', a)
            if m:
                start, end = int(m.group(1)), m.group(2)
                end = None if end == '$' else int(end) if end else start
                return [('ranged', f, (start, end)) for f in files]
        if any(re.match(r'^/.*/,/.*/p$', a) for a in args):
            return [('excerpt', f, None) for f in files]
        return [('other', f, None) for f in files]
    if cmd == 'head':
        n = 10
        for i, a in enumerate(args):
            m = re.match(r'^-n?(\d+)$', a)
            if m:
                n = int(m.group(1))
            elif a == '-n' and i + 1 < len(args) and args[i + 1].isdigit():
                n = int(args[i + 1])
        return [('ranged', f, (1, n)) for f in files]
    if cmd == 'tail':
        return [('ranged', f, None) for f in files]
    if cmd == 'awk':
        kind = 'excerpt' if any(re.search(r'/.+/\s*,\s*/', a) for a in args) else 'other'
        return [(kind, f, None) for f in files]
    if cmd in ('grep', 'rg', 'egrep'):
        ctx = any(re.match(r'^-(?:[A-Za-z]*[ABC]\d*|-(?:after|before)?-?context)', a) for a in args)
        return [('excerpt' if ctx else 'search', f, None) for f in files]
    if cmd in ('wc', 'ls', 'diff', 'git', 'cmp', 'shasum', 'md5', 'stat', 'test', '[', 'cp', 'find'):
        return []
    return [('other', f, None) for f in files]


def bash_reads(command):
    out = []
    for toks in segments(command):
        toks = [t for t in toks if not re.match(r'^\d?[<>]', t)]
        while toks and (re.match(r'^[A-Z_]+=', toks[0]) or toks[0] in ('then', 'do', 'else', '(', '{', 'time')):
            toks = toks[1:]
        out += classify_command(toks)
    return out


# -- reading a transcript --------------------------------------------------------------------

def tool_reads(path, seen=None):
    """Yield one dict per prelude read in a Claude Code transcript (session log or stream-json)."""
    seen = set() if seen is None else seen
    pending = {}
    with open(path, encoding='utf-8', errors='replace') as fh:
        for line in fh:
            try:
                e = json.loads(line)
            except ValueError:
                continue
            content = (e.get('message') or {}).get('content')
            if not isinstance(content, list):
                continue
            ts, cwd = e.get('timestamp', ''), e.get('cwd', '')
            for b in content:
                if not isinstance(b, dict):
                    continue
                if b.get('type') == 'tool_use' and b.get('id') not in seen:
                    name, inp = b.get('name'), b.get('input') or {}
                    base = dict(session=os.path.basename(path), ts=ts, cwd=cwd)
                    if name == 'Read' and rel(inp.get('file_path')):
                        seen.add(b['id'])
                        pending[b['id']] = (base, inp)
                    elif name == 'Bash':
                        for kind, f, rng in bash_reads(inp.get('command', '')):
                            seen.add(b['id'])
                            start, end = rng or (None, None)
                            yield dict(base, tool='Bash', file=rel(f), arg=f, kind=kind,
                                       start=start, end=end, cmd=inp['command'])
                    elif name == 'Grep' and rel(inp.get('path')):
                        seen.add(b['id'])
                        ctx = any(k in inp for k in ('-A', '-B', '-C', 'context'))
                        yield dict(base, tool='Grep', file=rel(inp['path']), arg=inp['path'],
                                   kind='excerpt' if ctx else 'search', start=None, end=None,
                                   cmd=str(inp.get('pattern')))
                elif b.get('type') == 'tool_result' and b.get('tool_use_id') in pending:
                    base, inp = pending.pop(b['tool_use_id'])
                    off, lim = inp.get('offset'), inp.get('limit')
                    ranged = bool(off or lim)
                    start = (off or 1) if ranged else None
                    yield dict(base, tool='Read', file=rel(inp['file_path']), arg=inp['file_path'],
                               kind='ranged' if ranged else 'whole', start=start,
                               end=(start + lim - 1) if ranged and lim else None,
                               cmd='offset={} limit={}'.format(off, lim))


# -- resolving a read against the file as it stood --------------------------------------------

def git(*args):
    r = subprocess.run(['git'] + list(args), capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


@lru_cache(maxsize=None)
def toplevel(directory):
    out = git('-C', directory, 'rev-parse', '--show-toplevel') if os.path.isdir(directory) else None
    return out.strip() if out else None


@lru_cache(maxsize=None)
def file_at(repo, path, ts):
    sha = (git('-C', repo, 'rev-list', '-1', '--before=' + ts, 'HEAD') or '').strip()
    if sha:
        out = git('-C', repo, 'show', '{}:{}'.format(sha, path))
        if out is not None:
            return tuple(out.splitlines())
    return None


def lines_for(r):
    full = os.path.normpath(os.path.join(r['cwd'] or '', r['arg']))
    top = toplevel(os.path.dirname(full)) or toplevel(r['cwd'] or '/nonexistent')
    got = file_at(top, os.path.relpath(full, top), r['ts']) if top else None
    m = re.search(r'(?:^|/)(yidam/prelude/.+)$', full)
    if not got and m:
        got = file_at(TEMPLATE, m.group(1), r['ts'])
    return got


def classify_mid(lines, s, e):
    heads = [j + 1 for j, l in enumerate(lines) if HEADING.match(l)]
    starts = any(s <= h <= s + 2 for h in heads)
    anchor = next((h for h in heads if s <= h <= s + 2), max([h for h in heads if h <= s], default=None))
    ends = False
    if anchor:
        level = len(HEADING.match(lines[anchor - 1]).group(1))
        nxt = next((h for h in heads if h > anchor and len(HEADING.match(lines[h - 1]).group(1)) <= level),
                   len(lines) + 1)
        ends = (nxt - 1) - 3 <= e <= (nxt - 1) + 3
    return 'one section' if starts and ends else 'starts at #' if starts else 'ends at #' if ends else 'window'


# -- census ----------------------------------------------------------------------------------

def default_log_dirs():
    """Every project's logs except this repository's own and anything run from a temp
    directory — which is where `section-reads-trial.sh` puts its clones, and where their
    sessions would otherwise be counted as corpus work."""
    root = os.path.expanduser('~/.claude/projects')
    own = re.sub(r'[/.]', '-', TEMPLATE)
    temp = re.compile(r'^-(?:private-)?(?:tmp|var-folders)-')
    return sorted(d for d in glob.glob(os.path.join(root, '*'))
                  if not os.path.basename(d).startswith(own) and not temp.match(os.path.basename(d)))


def census(dirs):
    seen, rows, sessions = set(), [], 0
    for d in dirs:
        for path in glob.glob(os.path.join(d, '**', '*.jsonl'), recursive=True):
            sessions += 1
            rows.extend(tool_reads(path, seen))

    paged, runs = set(), collections.defaultdict(list)
    for i, r in enumerate(rows):
        if r['kind'] == 'ranged':
            runs[(r['session'], r['file'])].append(i)
    for idx in runs.values():
        idx.sort(key=lambda i: rows[i]['start'] or 0)
        for a, b in zip(idx, idx[1:]):
            if rows[a]['end'] and rows[b]['start'] and abs(rows[b]['start'] - rows[a]['end']) <= 2:
                paged.update((a, b))

    table = collections.defaultdict(collections.Counter)
    for i, r in enumerate(rows):
        t = table[r['file']]
        if r['kind'] != 'ranged':
            t[r['kind']] += 1
        elif i in paged:
            t['whole'] += 1
        else:
            lines = lines_for(r)
            if lines is None:
                t['unresolved'] += 1
                continue
            n, s = len(lines), r['start'] or 1
            e = min(r['end'] or n, n)
            if s <= 1 and e >= n - 3:
                t['whole'] += 1
            elif s <= 1:
                t['head'] += 1
            else:
                t['mid-file'] += 1
                t[classify_mid(lines, s, e)] += 1

    cols = ['whole', 'head', 'mid-file', 'starts at #', 'one section', 'excerpt', 'search', 'other', 'unresolved']
    print('{} sessions in {} log directories; {} prelude reads ({} by the Read tool)\n'.format(
        sessions, len(dirs), len(rows), sum(1 for r in rows if r['tool'] == 'Read')))
    print('{:34}'.format('file') + ''.join('{:>13}'.format(c) for c in cols))
    total = collections.Counter()
    for f in sorted(table, key=lambda f: -sum(table[f].values())):
        total.update(table[f])
        print('{:34}'.format(f[:34]) + ''.join('{:>13}'.format(table[f][c]) for c in cols))
    print('{:34}'.format('all prelude files') + ''.join('{:>13}'.format(total[c]) for c in cols))


# -- trial -----------------------------------------------------------------------------------

def section_bounds(lines, title):
    """First and last line of `title`'s section. Fenced code is skipped: a YAML `# comment`
    inside a block is not a heading."""
    start, fenced = None, False
    for i, l in enumerate(lines, 1):
        if l.startswith('```'):
            fenced = not fenced
        elif not fenced and start is None and l.strip() == title:
            start = i
        elif not fenced and start and re.match(r'^#{1,%d}\s' % title.count('#', 0, title.index(' ')), l):
            return start, i - 1
    return start, len(lines)


def trial(runs_dir, file_rel='GRAPH.md', title='## The class contract'):
    for path in sorted(glob.glob(os.path.join(runs_dir, '*.jsonl'))):
        name = os.path.basename(path)[:-len('.jsonl')]
        lines = open(os.path.join(runs_dir, name, '.yidam/.vendor/prelude', file_rel)).read().splitlines()
        s0, e0 = section_bounds(lines, title)
        result, wrote = {}, False
        with open(path) as fh:
            for line in fh:
                e = json.loads(line) if line.strip().startswith('{') else {}
                if e.get('type') == 'result':
                    result = e
                for b in (e.get('message') or {}).get('content') or []:
                    if isinstance(b, dict) and b.get('name') in ('Write', 'Edit') and \
                            (b.get('input') or {}).get('file_path', '').endswith('.ont.yml'):
                        wrote = True
        print('\n=== {}  turns={} cost=${:.2f} wrote_class={}  ({} is lines {}-{} of {})'.format(
            name, result.get('num_turns'), result.get('total_cost_usd') or 0, wrote, title, s0, e0, len(lines)))
        for r in tool_reads(path):
            tag = ''
            if r['file'] == file_rel and r['kind'] in ('whole', 'ranged'):
                s, e = r['start'], min(r['end'] or len(lines), len(lines))
                if s is None or (s <= 1 and e >= len(lines) - 3):
                    tag = 'whole file'
                elif abs(s - s0) <= 2 and abs(e - e0) <= 3:
                    tag = 'the section'
                elif s >= s0 - 2 and e <= e0 + 3:
                    tag = 'inside it'
                else:
                    tag = 'other range'
            elif r['file'] == file_rel and r['kind'] == 'excerpt':
                tag = 'excerpt: read its output'
            print('  {:28} {:5} {:8} {:>5}-{:<5} {:24} {!r}'.format(
                r['file'][:28], r['tool'], r['kind'], str(r['start']), str(r['end']), tag, r['cmd'][:90]))


if __name__ == '__main__':
    if len(sys.argv) >= 2 and sys.argv[1] == 'census':
        census(sys.argv[2:] or default_log_dirs())
    elif len(sys.argv) == 3 and sys.argv[1] == 'trial':
        trial(sys.argv[2])
    else:
        sys.exit(__doc__)
