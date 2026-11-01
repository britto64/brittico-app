#!/usr/bin/env node
/* Imprime, para cada seletor, onde a peça ficou e o estilo que o navegador calculou (fundo, borda, letra), na página
   do site aberta com os dados de mentira do site-mock.cjs. Serve para copiar uma tela para o app sem adivinhar.

   node tools/site-estilos.cjs "/livepixel/" ".lp-lb, .lp-lb__item"      (CLICAR="seletor; seletor" antes de medir) */
const { open } = require("./site-mock.cjs");
const [, , PAGE, SELS] = process.argv;
open(PAGE, async (page) => {
  for (const sel of (process.env.CLICAR || "").split(";").map((x) => x.trim()).filter(Boolean)) {
    await page.click(sel);
    await page.waitForTimeout(400);
  }
  const N = Number(process.env.N || 3);
  const out = await page.evaluate(([sels, N]) => sels.map((sel) => [...document.querySelectorAll(sel)].slice(0, N).map((e) => {
    const r = e.getBoundingClientRect();
    const c = getComputedStyle(e);
    const pick = (k) => c[k];
    const f = (v) => Math.round(v * 10) / 10;
    let s = `${sel}  [${f(r.x)} ${f(r.y)} ${f(r.width)}×${f(r.height)}]`;
    const bits = {
      bg: pick("backgroundColor"), bd: c.borderTopWidth !== "0px" ? `${c.borderTopWidth} ${c.borderTopStyle} ${c.borderTopColor}` : "",
      rad: c.borderRadius !== "0px" ? c.borderRadius : "", color: pick("color"),
      font: `${c.fontWeight} ${c.fontSize}/${c.lineHeight} ${c.fontFamily.split(",")[0]}`, ls: c.letterSpacing !== "normal" ? c.letterSpacing : "",
      tt: c.textTransform !== "none" ? c.textTransform : "", pad: c.padding, op: c.opacity !== "1" ? c.opacity : "",
      sh: c.boxShadow !== "none" ? c.boxShadow : "", txt: (e.childElementCount === 0 ? e.textContent.trim().slice(0, 30) : ""),
    };
    for (const [k, v] of Object.entries(bits)) if (v && v !== "rgba(0, 0, 0, 0)") s += `  ${k}=${v}`;
    return s;
  }).join("\n")).join("\n"), [SELS.split(",").map((s) => s.trim()), N]);
  console.log(out);
});
