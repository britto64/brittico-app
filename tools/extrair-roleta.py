#!/usr/bin/env python3
"""Copia a roleta do LivePixel (o Roletasso, desenhado em SVG no livepixel/index.html) para assets/livepixel/.

O CSS do site pinta as lâmpadas, os pinos e os números por classe; aqui as cores entram direto no desenho:
  roleta-moldura-a.svg e -b.svg   a moldura nos dois tempos do pisca (as lâmpadas b0 acesas e as b1 apagadas, e o contrário)
  roleta-roda.svg                 o disco com as fatias, sem os números (o app escreve com a letra do jogo)
  roleta-ponteiro.svg
python3 tools/extrair-roleta.py [pasta public do site]
"""
import re, sys, pathlib
site = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "/mnt/movespeed/claude/1.servidor/1.site_brittico/public")
out = pathlib.Path(__file__).resolve().parent.parent / "assets" / "livepixel"
out.mkdir(parents=True, exist_ok=True)
html = (site / "livepixel/index.html").read_text()
def svg_at(marker):
    i = html.index(marker)
    j = html.index("</svg>", i) + 6
    return html[i:j]
def fix(svg):
    svg = re.sub(r'<svg class="[^"]*"', '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="300" height="300"', svg, count=1)
    return svg.replace(' aria-hidden="true"', "")
ACESA, APAGADA = "#ffcd75", "#7a5a2a"
frame = fix(svg_at('<svg class="lp-rl-frame"'))
for nome, b0, b1 in (("a", ACESA, APAGADA), ("b", APAGADA, ACESA)):
    f = frame.replace('class="lp-rl-bulb b0"', f'fill="{b0}" stroke="#1a1c2c" stroke-width="1.5"').replace('class="lp-rl-bulb b1"', f'fill="{b1}" stroke="#1a1c2c" stroke-width="1.5"')
    (out / f"roleta-moldura-{nome}.svg").write_text(f)
roda = fix(svg_at('<svg class="lp-rl-wheel-svg"'))
roda = re.sub(r"<text.*?</text>", "", roda, flags=re.S)
roda = roda.replace('class="lp-rl-peg"', 'fill="#fff" stroke="#1a1c2c" stroke-width="1.5"').replace('href="#rlSpark"', 'xlink:href="#rlSpark"')
(out / "roleta-roda.svg").write_text(roda)
i = html.index('class="lp-rl-pointer"')
ptr = html[html.index("<svg", i): html.index("</svg>", i) + 6]
if "xmlns" not in ptr:
    ptr = ptr.replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg"', 1)
(out / "roleta-ponteiro.svg").write_text(ptr)
# os números de cada fatia: posição, giro e tamanho, para o app escrever
for m in re.finditer(r'<text class="lp-rl-label" x="([\d.]+)" y="([\d.]+)" font-size="(\d+)".*?rotate\(([-\d.]+) .*?>([^<]+)</text>', html):
    print(m.groups())
