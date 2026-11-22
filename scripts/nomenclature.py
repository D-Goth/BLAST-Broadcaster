#!/usr/bin/env python3
import os
import re
import sys
import tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFS = os.path.join(ROOT, "docs", "nomenclature.toml")
DOC = os.path.join(ROOT, "docs", "NOMENCLATURE.md")
SOURCES = ["crates", "app/src-tauri/src", "app/ui"]
EXTENSIONS = (".rs", ".js", ".css", ".html")
TAG = re.compile(r"^\s*(?://|/\*|<!--)\s*\[([A-Z]+-\d{2,3})\]\s*(?:\*/|-->)?\s*$")
COMMENT = re.compile(r"^\s*(//|/\*|\*/|<!--)")
EXCERPT_LINES = 12
LANG = {".rs": "rust", ".js": "js", ".css": "css", ".html": "html"}


def source_files():
    for base in SOURCES:
        for dirpath, dirnames, filenames in os.walk(os.path.join(ROOT, base)):
            dirnames[:] = [d for d in dirnames if d not in ("target", "gen", "node_modules")]
            for name in sorted(filenames):
                if name.endswith(EXTENSIONS):
                    yield os.path.join(dirpath, name)


def excerpt(lines, start, ext):
    out, depth, opened = [], 0, False
    for line in lines[start:]:
        if TAG.match(line):
            break
        out.append(line)
        if ext == ".html":
            if len(out) >= 6:
                break
            continue
        code = re.sub(r'"(?:\\.|[^"\\])*"', '""', line)
        depth += code.count("{") - code.count("}")
        opened = opened or "{" in code
        if (opened and depth <= 0) or (not opened and code.rstrip().endswith(";")):
            break
        if len(out) >= EXCERPT_LINES:
            break
    while out and not out[-1].strip():
        out.pop()
    complete = (opened and depth <= 0) or (not opened and out and out[-1].rstrip().endswith(";")) or ext == ".html"
    if len(out) >= EXCERPT_LINES and not complete:
        indent = re.match(r"\s*", out[0]).group(0)
        out.append(f"{indent}…")
    width = min((len(l) - len(l.lstrip()) for l in out if l.strip()), default=0)
    return "\n".join(l[width:] for l in out)


def scan():
    tags, problems = {}, []
    for path in source_files():
        rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
        ext = os.path.splitext(path)[1]
        lines = open(path, encoding="utf-8").read().split("\n")
        in_tests = False
        for i, line in enumerate(lines):
            if line.strip() == "#[cfg(test)]" and lines[i + 1 : i + 2] and re.match(r"\s*mod \w+ \{", lines[i + 1]):
                in_tests = True
            m = TAG.match(line)
            if m:
                code = m.group(1)
                if code in tags:
                    problems.append(f"{rel}:{i + 1} : code {code} déjà utilisé dans {tags[code]['file']}")
                tags[code] = {"file": rel, "line": i + 1, "excerpt": excerpt(lines, i + 1, ext), "lang": LANG[ext]}
            elif COMMENT.match(line) and not in_tests:
                problems.append(f"{rel}:{i + 1} : commentaire hors nomenclature : {line.strip()[:60]}")
    return tags, problems


def sort_key(code, order):
    prefix, num = code.rsplit("-", 1)
    return (order.get(prefix, 999), int(num))


def render(defs, tags):
    prefixes = defs.get("prefixes", {})
    order = {p: i for i, p in enumerate(prefixes)}
    codes = defs.get("codes", {})
    out = [
        "# Nomenclature de BLAST Broadcaster",
        "",
        "Le code ne contient pas de commentaires explicatifs : chaque élément important porte un code de référence",
        "(`// [MIX-12]` en Rust et en JavaScript, `/* [STY-03] */` en CSS, `<!-- [HTM-02] -->` en HTML).",
        "Ce document associe chaque code à sa définition et à un extrait du code d'origine.",
        "",
        "Fichier généré par `python3 scripts/nomenclature.py` à partir de `docs/nomenclature.toml` :",
        "ne pas le modifier à la main.",
        "",
        "## Règles",
        "",
        "- Un code par unité : type, fonction, méthode métier, commande, module, section de style ou zone de page.",
        "- Les accesseurs simples et les implémentations de traits standards (Display, Error, From, Default, Drop)",
        "  relèvent du code de leur type.",
        "- Un nouveau code prend le numéro suivant de son préfixe ; un code n'est jamais réattribué.",
        "  Un élément supprimé garde sa définition dans `nomenclature.toml` avec `retire = true`.",
        "- Les tests ne sont pas codés.",
        "- `python3 scripts/nomenclature.py --check` vérifie que chaque code est défini, que chaque définition",
        "  est utilisée, qu'aucun commentaire libre n'est présent et que ce document est à jour.",
        "",
        "## Préfixes",
        "",
        "| Préfixe | Périmètre | Codes |",
        "|---|---|---|",
    ]
    for p, desc in prefixes.items():
        n = sum(1 for c in tags if c.startswith(p + "-"))
        out.append(f"| [`{p}`](#{p.lower()}) | {desc} | {n} |")
    current = None
    for code in sorted(codes, key=lambda c: sort_key(c, order)):
        d = codes[code]
        prefix = code.rsplit("-", 1)[0]
        if prefix != current:
            current = prefix
            out += ["", f"## {prefix}", "", prefixes.get(prefix, ""), ""]
        t = tags.get(code)
        title = d.get("titre", "")
        if d.get("retire"):
            out += [f"### [{code}] {title} *(retiré)*", "", d.get("definition", ""), ""]
            continue
        out += [
            f"### [{code}] {title}",
            "",
            d.get("definition", ""),
            "",
            f"`{t['file']}` ligne {t['line']}" if t else "*(introuvable)*",
            "",
        ]
        if t:
            out += [f"```{t['lang']}", t["excerpt"], "```", ""]
    return "\n".join(out).rstrip() + "\n"


def main():
    check = "--check" in sys.argv
    with open(DEFS, "rb") as f:
        defs = tomllib.load(f)
    tags, problems = scan()
    codes = defs.get("codes", {})
    prefixes = defs.get("prefixes", {})
    for code, t in sorted(tags.items()):
        if code not in codes:
            problems.append(f"{t['file']}:{t['line']} : code {code} sans définition dans docs/nomenclature.toml")
        elif codes[code].get("retire"):
            problems.append(f"{t['file']}:{t['line']} : code {code} marqué retiré mais encore utilisé")
        if code.rsplit("-", 1)[0] not in prefixes:
            problems.append(f"{t['file']}:{t['line']} : préfixe inconnu pour {code}")
    for code, d in sorted(codes.items()):
        if code not in tags and not d.get("retire"):
            problems.append(f"docs/nomenclature.toml : {code} n'apparaît plus dans le code (le marquer retire = true)")
    doc = render(defs, tags)
    if check:
        current = open(DOC, encoding="utf-8").read() if os.path.exists(DOC) else ""
        if current != doc:
            problems.append("docs/NOMENCLATURE.md n'est pas à jour : lancer python3 scripts/nomenclature.py")
    else:
        with open(DOC, "w", encoding="utf-8", newline="\n") as f:
            f.write(doc)
    for p in problems:
        print(p)
    print(f"{len(tags)} codes, {len(problems)} problème(s)")
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
