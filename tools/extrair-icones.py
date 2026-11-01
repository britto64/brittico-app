#!/usr/bin/env python3
"""Copia os ícones do site (shell.js, call/js/icons.js e os da home: feed-core.js e home-panels.js) para assets/icons/, como .svg brancos.

O app pinta cada ícone na cor que quiser (tint), então o `currentColor` do site vira branco.
Rodar de novo sempre que o site mudar um ícone:  python3 tools/extrair-icones.py <pasta public do site>
"""
import re, sys, pathlib

site = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "/mnt/movespeed/claude/1.servidor/1.site_brittico/public")
out = pathlib.Path(__file__).resolve().parent.parent / "assets" / "icons"
out.mkdir(parents=True, exist_ok=True)
for f in out.glob("*.svg"):
    f.unlink()

def save(name, svg):
    svg = svg.replace("currentColor", "#fff")
    if "xmlns" not in svg:
        svg = svg.replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg"', 1)
    (out / f"{name}.svg").write_text(svg)

shell = (site / "assets/js/shell.js").read_text()
wrap = lambda paths, w="2": f'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round">{paths}</svg>'
# a trilha: cada item do NAV com o seu ícone
for m in re.finditer(r'id:"([\w-]+)".*?icon:\s*(?:icon\(\'(.*?)\'\)|\'(<svg.*?</svg>)\')', shell, re.S):
    name, paths, raw = m.groups()
    save("rail-" + name, wrap(paths) if paths else raw)
for name, var in (("sun", "ic-sun"), ("moon", "ic-moon")):
    m = re.search(r'<svg class="' + var + r'"[^>]*>(.*?)</svg>', shell)
    save(name, wrap(m.group(1)))
m = re.search(r"SEARCH_SVG = '<svg[^>]*>(.*?)</svg>'", shell)
save("search", wrap(m.group(1)))

# a Call: o mapa P de icons.js (traço 1,75)
calls = (site / "call/js/icons.js").read_text()
block = calls[calls.index("const P = {"):]
block = block[: block.index("\n};")]
n = 0
for m in re.finditer(r'^\s*"?([\w-]+)"?:\s*((?:\'[^\']*\'\s*\+?\s*)+),?\s*$', block, re.M):
    name, parts = m.groups()
    paths = "".join(re.findall(r"'([^']*)'", parts))
    save(name, wrap(paths, "1.75"))
    n += 1
# os cargos de fábrica do mock da Call (o desenho de cada um em ROLES, traço 2)
mock = (site / "call/js/mock.js").read_text()
for key in ("admin", "streamer", "treinador", "jogador"):
    m = re.search(r'key: "' + key + r'".*?glyph: \'(.*?)\'', mock)
    save("role-" + key, wrap(m.group(1)))

# a home: os <svg> soltos no meio do HTML que o feed-core.js e o home-panels.js montam, achados por um pedaço do desenho
home = (site / "assets/js/feed-core.js").read_text() + (site / "assets/js/home-panels.js").read_text()
HOME = {
    "cw-image": '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5"',
    "cw-user": '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
    "cw-moon": 'M21 12.8A9',
    "cw-logout": 'M9 21H5a2',
    "vote-up": 'M12 5v14M5 12l7-7 7 7',
    "vote-down": 'M12 19V5M5 12l7 7 7-7',
    "post-more": '<circle cx="5" cy="12" r="1.7"/>',
    "post-moment": '<rect x="2" y="4" width="20" height="16" rx="3"/>',
    "post-comments": 'M21 11.5a8.4',
    "post-reply": 'M9 17 4 12l5-5',
    "post-flag": 'M4 15s1-1 4-1',
    "post-trash": 'M3 6h18M8 6V4',
    "pf-chevron": 'M9 18l6-6-6-6',
}
for name, mark in HOME.items():
    m = re.search(r"<svg(?:(?!</svg>).)*?" + re.escape(mark) + r"(?:(?!</svg>).)*</svg>", home, re.S)
    if not m:
        sys.exit(f"não achei o ícone {name} ({mark})")
    svg = m.group(0)
    svg = svg[svg.rindex("<svg"):]   # o <svg> mais perto do pedaço, não o primeiro do arquivo
    head, rest = svg.split(">", 1)   # o tamanho e a classe saem só da tag <svg>, não do <rect> de dentro
    svg = re.sub(r'\s(class|width|height|aria-hidden)="[^"]*"', "", head) + ">" + rest
    save(name, svg)
# o perfil (/u/): o ícone de cada aba do menu da esquerda, os das plataformas e o "verificada"
perfil = (site / "u/index.html").read_text()
abas = 0
for m in re.finditer(r'data-profile-tab="([\w-]+)">\s*(<svg.*?</svg>)', perfil, re.S):
    save("pf-tab-" + m.group(1), m.group(2))
    abas += 1
icones = perfil[perfil.index("var ICON = {"):]
icones = icones[: icones.index("};")]
for m in re.finditer(r"(\w+):\s*'(<svg.*?</svg>)'", icones):
    save("plat-" + m.group(1), m.group(2))
# as preferências: os três temas, o "Personalizada" da cor de destaque e o olho da prévia
for m in re.finditer(r'data-theme-val="(\w+)">(<svg.*?</svg>)', perfil, re.S):
    save("pf-theme-" + m.group(1), m.group(2))
for name, mark in (("pf-custom", 'id="btn-custom-color">'), ("pf-eye", 'class="edit-preview-header">')):
    m = re.search(re.escape(mark) + r"\s*(<svg.*?</svg>)", perfil, re.S)
    save(name, m.group(1))
m = re.search(r"var CHECK = '(<svg.*?</svg>)'", perfil)
save("plat-check", re.sub(r'\sclass="[^"]*"', "", m.group(1)))
# o LivePixel: cada <svg> que vem logo depois de um elemento com id (os botões do HUD, do menu e das janelas)
lp = (site / "livepixel/index.html").read_text()
nlp = 0
for m in re.finditer(r'id="(\w+)"[^>]*>\s*(?:<span[^>]*>[^<]*</span>\s*)?(<svg.*?</svg>)', lp, re.S):
    save("lp-" + m.group(1), m.group(2))
    nlp += 1
print(f"{nlp} ícones do LivePixel")
print(f"{len(list(out.glob('*.svg')))} ícones em {out} ({n} da Call, {len(HOME)} da home, {abas} abas do perfil)")
