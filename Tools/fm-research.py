#!/usr/bin/env python3
"""Read-only field-research helper for fm-editor.

Talks to a debug editor's fm-debug MCP server (``--mcp-only`` or the GUI) and
never writes game memory. It replaces ad-hoc probing with repeatable steps:

  status                      attached game, build and entity counts
  find TYPE QUERY             search entities by name or UID
  leads ENTITY [--from VER]   offsets mapped in VER but not in the attached
                              build, translated through shared anchor fields
  check ENTITY LOC:ENC W=V..  read one candidate for several witnesses and
                              compare with values read from FM's own screens
  scan ENTITY ENC W=V..       offsets (struct or --in SUB) where every
                              witness matches its expected value
  survey ENTITY LOC:ENC       value distribution across the whole array
  ui-search NAME              type NAME into FM's search box (headless FM)
  ui-menu TAB [ITEM]          open a profile menu item (--then TEXT clicks more)
  ui-holiday [--days N]       Calendar > Go On Holiday for N days (no inbox prompts)
  ui-shot [--crop X,Y,W,H]    screenshot FM's headless display, OCR the crop

Locations: ``0x204`` (entity offset), ``contract+0x3C`` (attached schema's
sub-record), ``0x390>0xC0`` (dereference 0x390, read at +0xC0; chains may be
longer). Encodings: u8 u16 i16 u32 i32 u64 ptr date fmstr bits8.
Witnesses: ``"Harry Kane=2023-08-12"`` or ``"uid:28049320=9"``; ``=-`` means
"expect missing/zero". Dates are YYYY-MM-DD.

Evidence still comes from FM's own UI: this tool narrows candidates and checks
them, it does not verify a mapping by itself (see docs/development.md).
"""
from __future__ import annotations

import argparse
import collections
import datetime as dt
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VALID = lambda p: p is not None and 0x10000 < p < 0x7FFFFFFFFFFF  # noqa: E731
STRUCT_READ = {"player": 0x400, "staff": 0x600, "club": 0x200}
VERSION_DIR = {"FM24": "fm24", "FM26": "fm26", "FM27": "fm27"}


# --------------------------------------------------------------------------
# MCP client
# --------------------------------------------------------------------------
class Mcp:
    def __init__(self, port: int):
        self.url = f"http://127.0.0.1:{port}/mcp"
        self.sid = None
        self.next_id = 0
        info = self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {},
                                       "clientInfo": {"name": "fm-research", "version": "1"}})
        if info is None:
            raise SystemExit("fm-debug did not answer initialize")
        self.rpc("notifications/initialized", notify=True)
        self.cache: dict[tuple[int, int], bytes | None] = {}

    def rpc(self, method, params=None, notify=False):
        body = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            body["params"] = params
        if not notify:
            self.next_id += 1
            body["id"] = self.next_id
        headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
        if self.sid:
            headers["Mcp-Session-Id"] = self.sid
        request = urllib.request.Request(self.url, json.dumps(body).encode(), headers)
        try:
            response = urllib.request.urlopen(request, timeout=180)
        except OSError as error:
            raise SystemExit(f"cannot reach fm-debug at {self.url}: {error}")
        self.sid = response.headers.get("Mcp-Session-Id") or self.sid
        text = response.read().decode()
        if notify or not text.strip():
            return None
        if text.lstrip().startswith("{"):
            return json.loads(text)
        for line in text.splitlines():
            if line.startswith("data:"):
                data = json.loads(line[5:])
                if "id" in data:
                    return data
        return None

    def call(self, name, **args) -> str:
        reply = self.rpc("tools/call", {"name": name, "arguments": args})
        if reply is None or "error" in reply:
            raise RuntimeError(json.dumps(reply and reply.get("error")))
        content = reply["result"]
        text = "\n".join(c.get("text", "") for c in content.get("content", []))
        if content.get("isError"):
            raise RuntimeError(text)
        return text

    def json(self, name, **args):
        return json.loads(self.call(name, **args))

    def read(self, addr: int, size: int) -> bytes | None:
        """Bytes at addr, or None if unreadable. Cached per process run."""
        key = (addr, size)
        if key in self.cache:
            return self.cache[key]
        if not VALID(addr):
            self.cache[key] = None
            return None
        out = bytearray()
        try:
            for start in range(0, size, 4096):
                text = self.call("dump_struct", address=addr + start, size=min(4096, size - start))
                out += parse_dump(text)
        except RuntimeError:
            self.cache[key] = None
            return None
        data = bytes(out[:size]) if len(out) >= size else None
        self.cache[key] = data
        return data

    def u64(self, addr):
        data = self.read(addr, 8)
        return int.from_bytes(data, "little") if data else None


def parse_dump(text: str) -> bytes:
    """Bytes from dump_struct's 'offset  hex hex .. ascii' grid (two hex groups per row)."""
    out = bytearray()
    for line in text.splitlines():
        parts = line.split("  ")
        if not parts or not re.fullmatch(r"[0-9A-Fa-f]{4,}", parts[0].strip()):
            continue
        tokens = []
        for part in parts[1:]:
            words = part.split()
            if words and all(re.fullmatch(r"[0-9A-F]{2}", w) for w in words):
                tokens += words
            elif words:
                break
        out += bytes.fromhex("".join(tokens[:16]))
    return bytes(out)


# --------------------------------------------------------------------------
# Schema source parsing (leads only; the attached schema stays authoritative)
# --------------------------------------------------------------------------
FIELD_RE = re.compile(
    r'Field\s*\{\s*key:\s*"(?P<key>[^"]+)",\s*label:\s*"(?P<label>[^"]*)",\s*entity:\s*(?P<entity>\w+),'
    r'\s*category:\s*(?P<category>\w+),\s*offset:\s*(?P<offset>.*?)\s*\}\s*,', re.S)
SUB_RE = re.compile(r'SubRecord\s*\{\s*id:\s*"(?P<id>[^"]+)",\s*parent:\s*SubRecordParent::'
                    r'(?:Direct|In\("(?P<parent>[^"]+)"\)),\s*ptr_offset:\s*(?P<ptr>0x[0-9A-Fa-f]+|\d+)')
OFFSET_RE = re.compile(r'(?P<ctor>\w+)\((?:"(?P<sub>[^"]+)",\s*)?(?P<off>0x[0-9A-Fa-f]+|\d+)')
ENTITY_PREFIX = {"Player": "player", "Staff": "staff", "Club": "club", "Stadium": "stadium"}


def schema(version_dir: str):
    fields, subs = {}, {}
    for path in sorted((ROOT / "src" / "fields" / version_dir).glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        entity_file = path.stem
        for m in FIELD_RE.finditer(text):
            off = OFFSET_RE.match(m["offset"]) if m["offset"] != "None" else None
            fields[m["key"]] = {
                "label": m["label"], "entity": ENTITY_PREFIX.get(m["entity"], m["entity"].lower()),
                "ctor": off["ctor"] if off else None, "sub": off["sub"] if off else None,
                "offset": int(off["off"], 0) if off else None}
        for m in SUB_RE.finditer(text):
            subs[(entity_file, m["id"])] = (m["parent"], int(m["ptr"], 0))
    return fields, subs


def encoding_of(ctor: str | None) -> str:
    if not ctor:
        return "?"
    if "date" in ctor:
        return "date"
    if "fm_string" in ctor:
        return "fmstr"
    if "pointer" in ctor:
        return "ptr"
    if "u32" in ctor:
        return "u32"
    if "u16" in ctor or ctor in ("rep",):
        return "u16"
    return "u8"


def translate(offset, anchors):
    """Lead offsets for `offset` from the nearest (source, target) anchor pairs."""
    if not anchors:
        return []
    ordered = sorted(anchors)
    below = [a for a in ordered if a[0] <= offset]
    above = [a for a in ordered if a[0] > offset]
    leads = []
    for anchor in (below[-1:] + above[:1]):
        lead = offset + (anchor[1] - anchor[0])
        if lead >= 0 and lead not in leads:
            leads.append(lead)
    return leads


# --------------------------------------------------------------------------
# Locations, encodings and witnesses
# --------------------------------------------------------------------------
class Ctx:
    def __init__(self, mcp: Mcp, version_dir: str):
        self.mcp = mcp
        self.fields, self.subs = schema(version_dir)
        self.shift = 0x358 if version_dir == "fm24" else 0  # staff human manager, unused for players

    def sub_ptr(self, entity, base, sub_id):
        parent, ptr_off = self.subs[(entity, sub_id)]
        root = base if parent is None else self.sub_ptr(entity, base, parent)
        if not VALID(root):
            return None
        target = self.mcp.u64(root + ptr_off)
        return target if VALID(target) else None

    def resolve(self, entity, base, loc: str):
        """Address to read for `loc`, or None when a pointer on the way is invalid."""
        if "+" in loc and not loc.startswith("0x"):
            sub_id, off = loc.split("+", 1)
            target = self.sub_ptr(entity, base, sub_id)
            return target + int(off, 0) if target else None
        hops = [int(h, 0) for h in loc.split(">")]
        addr = base
        for hop in hops[:-1]:
            addr = self.mcp.u64(addr + hop)
            if not VALID(addr):
                return None
        return addr + hops[-1]


SIZES = {"u8": 1, "bits8": 1, "u16": 2, "i16": 2, "u32": 4, "i32": 4, "date": 4, "u64": 8, "ptr": 8, "fmstr": 8}


def decode(mcp: Mcp, enc: str, data: bytes):
    if data is None:
        return None
    v = int.from_bytes(data, "little")
    if enc in ("u8", "u16", "u32", "u64"):
        return v
    if enc == "bits8":
        return f"0b{v:08b}"
    if enc == "i16":
        return v - (1 << 16) if v >= 1 << 15 else v
    if enc == "i32":
        return v - (1 << 32) if v >= 1 << 31 else v
    if enc == "ptr":
        return hex(v)
    if enc == "date":
        return date_text(int.from_bytes(data[:2], "little"), int.from_bytes(data[2:], "little"))
    if enc == "fmstr":
        return fm_string(mcp, v)
    raise SystemExit(f"unknown encoding {enc}")


def date_text(day, year):
    d = day & 0x1FF
    if not (1900 <= year <= 2200) or not (1 <= d <= 366):
        return f"raw({day:#x},{year})"
    try:
        value = dt.date(year, 1, 1) + dt.timedelta(days=d - 1)
    except OverflowError:
        return f"raw({day:#x},{year})"
    flags = f" flags={day >> 9:#x}" if day >> 9 else ""
    return value.isoformat() + flags


def fm_string(mcp: Mcp, ptr):
    """Decode an FM string object pointer using fm-debug's own fm_string decoder."""
    if not VALID(ptr):
        return None
    # probe_range decodes a pointer slot, so point it at a slot holding `ptr`:
    # reading the object directly is not possible, so fall back to raw ASCII.
    data = mcp.read(ptr, 0x40)
    if not data:
        return None
    text = re.findall(rb"[\x20-\x7e\xc0-\xff]{3,}", data)
    return text[0].decode("latin-1") if text else f"obj@{ptr:#x}"


def read_loc(ctx: Ctx, entity, base, loc, enc):
    addr = ctx.resolve(entity, base, loc)
    if addr is None:
        return None
    if enc == "fmstr":
        try:
            out = ctx.mcp.json("probe_range", base=addr, start_offset=0, end_offset=8, step=8, encoding="fm_string")
            return out[0].get("decoded")
        except (RuntimeError, ValueError, IndexError, KeyError):
            return None
    return decode(ctx.mcp, enc, ctx.mcp.read(addr, SIZES[enc]))


def witness(mcp: Mcp, entity, spec):
    who, _, expected = spec.partition("=")
    if who.startswith("uid:"):
        hits = mcp.json("search_entities", entity_type=entity, query=who[4:])
    elif who.startswith("idx:"):
        hits = [dict(mcp.json("get_entity_address", entity_type=entity, index=int(who[4:])), name=who)]
    else:
        hits = mcp.json("search_entities", entity_type=entity, query=who)
        exact = [h for h in hits if h.get("name", "").lower() == who.lower()]
        hits = exact or hits
    if not hits:
        raise SystemExit(f"witness not found: {who}")
    if len(hits) > 1 and not who.startswith(("uid:", "idx:")):
        print(f"note: {who!r} matched {len(hits)} entities; using {hits[0].get('name')} uid {hits[0].get('uid')}",
              file=sys.stderr)
    h = hits[0]
    return {"name": h.get("name", who), "uid": h.get("uid"), "addr": h["address"],
            "expected": expected if expected != "" else None}


def matches(value, expected):
    if expected is None:
        return None
    if expected == "-":
        return value in (None, 0, "0x0") or (isinstance(value, str) and value.startswith("raw("))
    if isinstance(value, int):
        try:
            return value == int(expected, 0)
        except ValueError:
            return False
    return value is not None and str(value).split(" flags")[0].lower() == expected.lower()


# --------------------------------------------------------------------------
# Commands
# --------------------------------------------------------------------------
def cmd_status(ctx, args):
    print(ctx.mcp.call("attach_status"))


def cmd_find(ctx, args):
    for hit in ctx.mcp.json("search_entities", entity_type=args.type, query=args.query):
        print(f"{hit.get('uid'):>12}  {hit['address']:#014x}  idx {hit.get('index'):>6}  {hit.get('name')}")


def cmd_leads(ctx, args):
    src_fields, _ = schema(args.source)
    target = ctx.fields
    anchors = collections.defaultdict(list)
    for key, f in src_fields.items():
        t = target.get(key)
        if f["offset"] is None or not t or t["offset"] is None:
            continue
        if f["sub"] == t["sub"] and f["entity"] == args.entity:
            anchors[f["sub"]].append((f["offset"], t["offset"]))
    print(f"{'key':24} {'label':24} {'src':>14} {'enc':6} leads in attached build")
    for key, f in sorted(src_fields.items(), key=lambda kv: (kv[1]["sub"] or "", kv[1]["offset"] or 0)):
        t = target.get(key)
        if f["entity"] != args.entity or f["offset"] is None or (t and t["offset"] is not None):
            continue
        leads = translate(f["offset"], anchors.get(f["sub"], []))
        where = f"{f['sub']}+" if f["sub"] else ""
        status = "absent" if not t else "unmapped"
        print(f"{key:24} {f['label'][:24]:24} {where + hex(f['offset']):>14} {encoding_of(f['ctor']):6} "
              f"{', '.join(where + hex(x) for x in leads) or '(no anchors)'}  [{status}]")


def cmd_check(ctx, args):
    loc, _, enc = args.loc.partition(":")
    enc = enc or "u8"
    ok = total = 0
    for spec in args.witnesses:
        w = witness(ctx.mcp, args.entity, spec)
        value = read_loc(ctx, args.entity, w["addr"], loc, enc)
        verdict = matches(value, w["expected"])
        if verdict is not None:
            total += 1
            ok += bool(verdict)
        mark = {True: "OK  ", False: "FAIL", None: "    "}[verdict]
        print(f"{mark} {w['name'][:28]:28} uid {str(w['uid']):>11}  {loc}:{enc} = {value!r}"
              + (f"  (FM: {w['expected']})" if w["expected"] else ""))
    if total:
        print(f"{ok}/{total} witnesses match")
    return 0 if ok == total else 1


def cmd_scan(ctx, args):
    enc = args.enc
    width = SIZES[enc]
    ws = [witness(ctx.mcp, args.entity, s) for s in args.witnesses]
    if any(w["expected"] is None for w in ws):
        raise SystemExit("scan needs an expected value for every witness")
    survivors = None
    for w in ws:
        base = ctx.sub_ptr(args.entity, w["addr"], args.within) if args.within else w["addr"]
        size = args.size or (0x100 if args.within else STRUCT_READ.get(args.entity, 0x400))
        data = ctx.mcp.read(base, size) if base else None
        if not data:
            print(f"{w['name']}: unreadable", file=sys.stderr)
            return 1
        hits = set()
        for off in range(0, size - width + 1, args.step):
            if matches(decode(ctx.mcp, enc, data[off:off + width]), w["expected"]):
                hits.add(off)
        survivors = hits if survivors is None else survivors & hits
        print(f"{w['name']}: {len(hits)} offsets match {w['expected']}; {len(survivors)} common so far",
              file=sys.stderr)
    where = f"{args.within}+" if args.within else ""
    for off in sorted(survivors or []):
        print(f"{where}{off:#x}")
    return 0 if survivors else 1


def cmd_survey(ctx, args):
    loc, _, enc = args.loc.partition(":")
    enc = enc or "u8"
    count = ctx.mcp.json("attach_status")[{"player": "players", "staff": "staff", "club": "clubs"}[args.entity]]
    step = max(1, count // args.samples) if args.samples else 1
    values = collections.Counter()
    invalid = 0
    for index in range(0, count, step):
        addr = ctx.mcp.json("get_entity_address", entity_type=args.entity, index=index)["address"]
        value = read_loc(ctx, args.entity, addr, loc, enc)
        if value is None:
            invalid += 1
        else:
            values[value if not isinstance(value, str) or enc != "date" else value[:4]] += 1
    seen = sum(values.values()) + invalid
    print(f"{seen} entities sampled (step {step}); unreadable/null chain {invalid}")
    for value, n in values.most_common(args.top):
        print(f"{n:>8}  {value!r}")
    if isinstance(next(iter(values), None), int):
        ints = [v for v in values.elements()]
        print(f"min {min(ints)}  max {max(ints)}  distinct {len(values)}")


def cmd_where(ctx, args):
    """Entities whose value at LOC satisfies a Python expression over `v` (e.g. 'v & 1')."""
    loc, _, enc = args.loc.partition(":")
    enc = enc or "u8"
    count = ctx.mcp.json("attach_status")[{"player": "players", "staff": "staff", "club": "clubs"}[args.entity]]
    found = 0
    for index in range(count):
        addr = ctx.mcp.json("get_entity_address", entity_type=args.entity, index=index)["address"]
        v = read_loc(ctx, args.entity, addr, loc, enc)
        if v is None or not eval(args.expr, {}, {"v": v}):  # noqa: S307 - local research tool
            continue
        name = "?"
        try:
            for f in ctx.mcp.json("read_entity_fields", entity_type=args.entity, address=addr,
                                  fields=[f"{args.entity[0]}_name"]):
                name = f.get("value") or name
        except (RuntimeError, ValueError, TypeError):
            pass
        print(f"idx:{index:<7} {v!r:>14}  {name}")
        found += 1
        if found >= args.limit:
            break
    return 0


def fmh_env():
    out = subprocess.run(["bash", str(ROOT / "Tools" / "fm-headless.sh"), "env"],
                         capture_output=True, text=True)
    env = dict(os.environ)
    for name, value in re.findall(r"(\w+)=(\S+)", out.stdout):
        env[name] = value
    if "DISPLAY" not in env or out.returncode:
        raise SystemExit("headless FM display is not running (bash Tools/fm-headless.sh start)")
    return env


def fm_click(env, x, y):
    for cmd in (["mousemove", str(x - 4), str(y - 3)], ["mousemove", str(x), str(y)],
                ["mousedown", "1"], ["mouseup", "1"]):
        subprocess.run(["xdotool", *cmd], env=env, check=True)
        subprocess.run(["sleep", "0.3"])


def ocr_rows(png, crop):
    """(text, centre x, centre y) per OCR line inside `crop` (x, y, w, h), screen coordinates."""
    x0, y0, w, h = crop
    with tempfile.TemporaryDirectory() as tmp:
        part = Path(tmp) / "crop.png"
        subprocess.run(["magick", png, "-crop", f"{w}x{h}+{x0}+{y0}", "-resize", "200%", str(part)], check=True)
        tsv = subprocess.run(["tesseract", str(part), "-", "--psm", "6", "tsv"],
                             capture_output=True, text=True).stdout
    lines = collections.OrderedDict()
    for row in tsv.splitlines()[1:]:
        cols = row.split("\t")
        if len(cols) < 12 or not cols[11].strip():
            continue
        key = (cols[2], cols[3], cols[4])
        left, top, width, height = (int(c) // 2 for c in cols[6:10])
        text, boxes = lines.get(key, ("", []))
        lines[key] = ((text + " " + cols[11]).strip(), boxes + [(cols[11], left, top, width, height)])
    out = []
    for text, boxes in lines.values():
        # Anchor on the first real word (OCR often prefixes row borders as "*", "f", "i").
        word = next((b for b in boxes if len(b[0]) > 1 and b[0][0].isalnum()), boxes[0])
        out.append((text, x0 + word[1] + word[3] // 2, y0 + word[2] + word[4] // 2))
    return out


def cmd_ui_search(ctx, args):
    """Type NAME into FM24's search box; with --open, click the result row OCR finds for NAME."""
    env = fmh_env()
    fm_click(env, 700, 33)           # focus the search box (top bar)
    subprocess.run(["sleep", "1"])
    subprocess.run(["xdotool", "key", "--repeat", "40", "BackSpace"], env=env, check=True)
    subprocess.run(["xdotool", "type", "--delay", "70", args.name], env=env, check=True)
    subprocess.run(["sleep", "5"])
    shot = args.out or tempfile.mktemp(prefix="fm-", suffix=".png")
    subprocess.run(["grim", shot], env=env, check=True)
    rows = ocr_rows(shot, (330, 40, 820, 400))
    if args.open is None:
        print(shot)
        for text, _, _ in rows:
            print(text)
        return 0
    # Skip FM's recent-search rows ("Home<query>"); take the open-th matching result.
    wanted = args.name.lower()
    hits = [r for r in rows if wanted in r[0].lower() and not re.search(r"home\s*" + re.escape(wanted), r[0].lower())]
    if len(hits) <= args.open:
        print(f"no result row {args.open} for {args.name!r}; OCR saw: {[r[0] for r in rows]}", file=sys.stderr)
        return 1
    text, x, y = hits[args.open]
    fm_click(env, x, y)
    subprocess.run(["sleep", "4"])
    subprocess.run(["grim", shot], env=env, check=True)
    print(f"opened {text!r}\n{shot}")
    return 0


def find_text(env, label, region, attempts=4, box=False):
    """Centre of `label` in region (x, y, w, h), or its (x, y, w, h) box with box=True.
    Retries while menus/pages are still animating."""
    for attempt in range(attempts):
        hit = find_text_once(env, label, region, box)
        if hit:
            return hit
        subprocess.run(["sleep", "1"])
    return None


def find_text_once(env, label, region, box=False):
    """Centre of the OCR word sequence matching `label` inside region (x, y, w, h)."""
    shot = tempfile.mktemp(prefix="fm-", suffix=".png")
    subprocess.run(["grim", shot], env=env, check=True)
    x0, y0, w, h = region
    with tempfile.TemporaryDirectory() as tmp:
        part = Path(tmp) / "crop.png"
        tsv = ""
        # Plain and grey-inverted passes: highlighted menu rows defeat a single pass.
        for extra in ([], ["-colorspace", "Gray", "-threshold", "55%"]):
            subprocess.run(["magick", shot, "-crop", f"{w}x{h}+{x0}+{y0}", "-resize", "200%", *extra, str(part)],
                           check=True)
            tsv += subprocess.run(["tesseract", str(part), "-", "--psm", "11", "tsv"],
                                  capture_output=True, text=True).stdout
    words = []
    for row in tsv.splitlines()[1:]:
        cols = row.split("\t")
        if len(cols) >= 12 and cols[11].strip() and cols[6].isdigit():
            left, top, width, height = (int(c) // 2 for c in cols[6:10])
            words.append((re.sub(r"[^a-z0-9&]", "", cols[11].lower()), x0 + left, y0 + top, width, height))
    target = [re.sub(r"[^a-z0-9&]", "", t) for t in label.lower().split()]
    for i in range(len(words) - len(target) + 1):
        if all(words[i + j][0] == t for j, t in enumerate(target)):
            first, last = words[i], words[i + len(target) - 1]
            if box:
                return first[1], first[2], last[1] + last[3] - first[1], first[4]
            return (first[1] + last[1] + last[3]) // 2, first[2] + first[4] // 2
    return None


def cmd_ui_menu(ctx, args):
    """Open a profile tab menu (e.g. History) and choose an item (e.g. Milestones)."""
    env = fmh_env()
    subprocess.run(["xdotool", "key", "Escape"], env=env, check=True)  # close any open dropdown
    subprocess.run(["sleep", "0.8"])
    pos = find_text(env, args.tab, (150, 62, 1300, 40))
    if not pos:
        print(f"tab {args.tab!r} not found", file=sys.stderr)
        return 1
    fm_click(env, *pos)
    subprocess.run(["sleep", "1.2"])
    if args.item:
        item = find_text(env, args.item, (pos[0] - 120, 85, 520, 420))
        if not item:
            print(f"menu item {args.item!r} not found", file=sys.stderr)
            return 1
        fm_click(env, *item)
        subprocess.run(["sleep", "2"])
    for extra in args.then or []:
        hit = find_text(env, extra, (150, 60, 1770, 1000))
        if not hit:
            print(f"{extra!r} not found", file=sys.stderr)
            return 1
        fm_click(env, *hit)
        subprocess.run(["sleep", "2"])
    return cmd_ui_shot(ctx, argparse.Namespace(crop=args.crop, out=args.out))


DATE_RE = re.compile(r"(\d{1,2})\s*([A-Z][a-z]{2})\s*(\d{4})")


def fm_date(env):
    """FM24's top-bar game date (e.g. '6 Jul 2023'), read by OCR, or None."""
    shot = tempfile.mktemp(prefix="fm-", suffix=".png")
    subprocess.run(["grim", shot], env=env, check=True)
    with tempfile.TemporaryDirectory() as tmp:
        part = Path(tmp) / "date.png"
        subprocess.run(["magick", shot, "-crop", "300x60+1600+10", "-colorspace", "Gray", "-negate",
                        "-resize", "300%", str(part)], check=True)
        text = subprocess.run(["tesseract", str(part), "-", "--psm", "6"], capture_output=True, text=True).stdout
    m = DATE_RE.search(text)
    return " ".join(m.groups()) if m else None


def screen_text(env, crop):
    """OCR of one tight single-line crop (x, y, w, h) of FM's display."""
    shot = tempfile.mktemp(prefix="fm-", suffix=".png")
    subprocess.run(["grim", shot], env=env, check=True)
    x, y, w, h = crop
    with tempfile.TemporaryDirectory() as tmp:
        part = Path(tmp) / "line.png"
        subprocess.run(["magick", shot, "-crop", f"{w}x{h}+{x}+{y}", "-resize", "300%", "-colorspace", "Gray",
                        "-negate", str(part)], check=True)
        return subprocess.run(["tesseract", str(part), "-", "--psm", "7"], capture_output=True, text=True).stdout


def expect(env, crop, words, what, attempts=5):
    """Wait until a crop of FM's screen reads all `words` (case-insensitive, spaces ignored)."""
    squash = lambda t: re.sub(r"\s+", "", t.lower())  # noqa: E731
    for _ in range(attempts):
        if all(squash(w) in squash(screen_text(env, crop)) for w in words):
            return True
        subprocess.run(["sleep", "1"])
    print(f"{what} not found on screen", file=sys.stderr)
    return False


# FM24 at 1920x1080: the top-bar date opens the calendar; its Go On Holiday
# button opens a centred dialog. Each click is checked against the expected
# text first, so a different screen fails instead of clicking blindly.
HOLIDAY_CALENDAR_BUTTON = (1725, 398, 160, 44)
HOLIDAY_DIALOG_TITLE = (800, 255, 240, 34)
HOLIDAY_AFTER_LABEL = (680, 676, 200, 30)
HOLIDAY_GO_BUTTON = (1040, 773, 150, 36)


def centre(crop):
    x, y, w, h = crop
    return x + w // 2, y + h // 2


def cmd_ui_holiday(ctx, args):
    """Advance FM by N days with Calendar > Go On Holiday (skips inbox/continue prompts)."""
    env = fmh_env()
    subprocess.run(["xdotool", "key", "Escape"], env=env, check=True)
    subprocess.run(["sleep", "0.8"])
    before = fm_date(env)
    fm_click(env, 1660, 32)
    if not expect(env, HOLIDAY_CALENDAR_BUTTON, ["Go On Holiday"], "calendar 'Go On Holiday' button"):
        return 1
    fm_click(env, *centre(HOLIDAY_CALENDAR_BUTTON))
    if not (expect(env, HOLIDAY_DIALOG_TITLE, ["Holiday Options"], "holiday dialog")
            and expect(env, HOLIDAY_AFTER_LABEL, ["Return", "after"], "'Return from holiday after' option")):
        return 1
    x, y, _, h = HOLIDAY_AFTER_LABEL
    fm_click(env, x - 6, y + h // 2)            # the radio button left of the label
    fm_click(env, 1014, y + h // 2)             # the day-count field
    subprocess.run(["xdotool", "key", "--repeat", "4", "BackSpace"], env=env, check=True)
    subprocess.run(["xdotool", "type", str(args.days)], env=env, check=True)
    subprocess.run(["sleep", "0.5"])
    if not expect(env, HOLIDAY_GO_BUTTON, ["Go On Holiday"], "dialog 'Go On Holiday' button"):
        return 1
    fm_click(env, *centre(HOLIDAY_GO_BUTTON))
    deadline = 60 + 20 * args.days
    for _ in range(deadline // 2):
        subprocess.run(["sleep", "2"])
        after = fm_date(env)
        if after and after != before:
            subprocess.run(["sleep", "3"])      # let the day's processing settle
            print(f"{before} -> {fm_date(env) or after}")
            return 0
    print(f"date still {before} after {deadline}s", file=sys.stderr)
    return 1


def cmd_ui_shot(ctx, args):
    env = fmh_env()
    shot = args.out or tempfile.mktemp(prefix="fm-", suffix=".png")
    subprocess.run(["grim", shot], env=env, check=True)
    print(shot)
    if args.crop and subprocess.run(["which", "tesseract"], capture_output=True).returncode == 0:
        x, y, w, h = (int(v) for v in args.crop.split(","))
        crop = shot.replace(".png", "-crop.png")
        subprocess.run(["magick", shot, "-crop", f"{w}x{h}+{x}+{y}", "-resize", "200%", crop], check=True)
        text = subprocess.run(["tesseract", crop, "-", "--psm", "6"], capture_output=True, text=True).stdout
        print(text.strip())
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, default=int(os.environ.get("FM_MCP_PORT", 0)) or None,
                        help="fm-debug port (default: .mcp-port, then 7799)")
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("status")
    p = sub.add_parser("find"); p.add_argument("type"); p.add_argument("query")
    p = sub.add_parser("leads"); p.add_argument("entity"); p.add_argument("--from", dest="source", default="fm27")
    p = sub.add_parser("check"); p.add_argument("entity"); p.add_argument("loc"); p.add_argument("witnesses", nargs="+")
    p = sub.add_parser("scan"); p.add_argument("entity"); p.add_argument("enc"); p.add_argument("witnesses", nargs="+")
    p.add_argument("--in", dest="within"); p.add_argument("--size", type=lambda v: int(v, 0))
    p.add_argument("--step", type=int, default=1)
    p = sub.add_parser("survey"); p.add_argument("entity"); p.add_argument("loc")
    p.add_argument("--samples", type=int, default=2000); p.add_argument("--top", type=int, default=15)
    p = sub.add_parser("where"); p.add_argument("entity"); p.add_argument("loc"); p.add_argument("expr")
    p.add_argument("--limit", type=int, default=25)
    p = sub.add_parser("ui-search"); p.add_argument("name"); p.add_argument("--open", type=int)
    p.add_argument("--crop"); p.add_argument("--out")
    p = sub.add_parser("ui-menu"); p.add_argument("tab"); p.add_argument("item", nargs="?")
    p.add_argument("--then", action="append", help="click this on-screen text afterwards (repeatable)")
    p.add_argument("--crop"); p.add_argument("--out")
    p = sub.add_parser("ui-holiday"); p.add_argument("--days", type=int, default=1)
    p = sub.add_parser("ui-shot"); p.add_argument("--crop"); p.add_argument("--out")
    args = parser.parse_args(argv)

    if args.cmd.startswith("ui-"):
        return {"ui-search": cmd_ui_search, "ui-shot": cmd_ui_shot, "ui-menu": cmd_ui_menu,
                "ui-holiday": cmd_ui_holiday}[args.cmd](None, args) or 0
    port = args.port
    if port is None:
        port_file = Path.cwd() / ".mcp-port"
        port = int(port_file.read_text().strip()) if port_file.exists() else 7799
    mcp = Mcp(port)
    status = mcp.json("attach_status")
    if not status.get("attached"):
        raise SystemExit("fm-debug is not attached to a game")
    version_dir = VERSION_DIR.get(status.get("version"))
    if not version_dir:
        raise SystemExit(f"no schema directory for {status.get('version')}")
    ctx = Ctx(mcp, version_dir)
    handler = {"status": cmd_status, "find": cmd_find, "leads": cmd_leads, "check": cmd_check,
               "scan": cmd_scan, "survey": cmd_survey, "where": cmd_where}[args.cmd]
    return handler(ctx, args) or 0


if __name__ == "__main__":
    sys.exit(main())
