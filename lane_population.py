#!/usr/bin/env python3
"""Establish the entry population of this lane's two registry files by parsing, never grepping.

Resolves every `pub const ID` in the slots tree so a binding row written as
`crate::slots::x::y::ID` counts as the id string it holds rather than as unresolved. An
unresolved row would be silently counted unbound and the population would read smaller
with nothing saying so, so the assert is load-bearing.
"""
import re
import sys
import tomllib
import pathlib

ROOT = pathlib.Path("/home/alex/pf-ws-breadth-reactive")
SLOTS = ROOT / "crates/plateforce-analysis/src/slots"
BIND = ROOT / "crates/plateforce-analysis/src/binding.rs"

consts = {}
for p in SLOTS.rglob("*.rs"):
    parts = list(p.relative_to(SLOTS).with_suffix("").parts)
    if parts[-1] == "mod":
        parts = parts[:-1]
    path = "crate::slots::" + "::".join(parts)
    for name, val in re.findall(r'pub const (\w+):\s*&str\s*=\s*"([^"]*)"', p.read_text()):
        consts[f"{path}::{name}"] = val

bind = BIND.read_text()
for name, val in re.findall(r'pub const (\w+):\s*&str\s*=\s*"([^"]*)"', bind):
    consts[name] = consts[f"crate::binding::{name}"] = val

bound, unresolved = set(), []
for m in re.finditer(r"^\s*id:\s*(.+?),\s*$", bind, re.M):
    raw = m.group(1).strip()
    if raw.startswith('"'):
        bound.add(raw.strip('"'))
    elif raw in consts:
        bound.add(consts[raw])
    else:
        unresolved.append(raw)
assert not unresolved, unresolved

files = sys.argv[1:] or ["reactive-strength.toml", "sprint-start.toml"]
for fname in files:
    doc = tomllib.loads((ROOT / "registry/methods" / fname).read_text())
    rows = doc.get("method", [])
    print(f"\n=== {fname}: {len(rows)} entries, top-level arrays {sorted(doc.keys())}")
    width = max((len(e["id"]) for e in rows), default=10)
    per_construct = {}
    for e in rows:
        b = "yes" if e["id"] in bound else "no"
        r = e.get("reach", {}).get("boundary", "")
        print(f'  {e["id"]:<{width}}  {e.get("construct", "-"):<28}  bnd={b:<3} reach={r}')
        per_construct.setdefault(e.get("construct", "-"), []).append(e["id"])
    nb = sum(e["id"] in bound for e in rows)
    nr = sum(1 for e in rows if e.get("reach"))
    print(f"  bound {nb} of {len(rows)}; declare a barrier {nr} of {len(rows)}")
    print(f"  constructs filled: {len(per_construct)}")
    for c, ids in sorted(per_construct.items()):
        print(f"    {c:<30} {len(ids)}")

print(f"\nbinding.rs rows resolved: {len(bound)}  unresolved: {len(unresolved)}")
