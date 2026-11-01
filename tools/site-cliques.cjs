#!/usr/bin/env node
/* Lista tudo o que dá para clicar ou digitar numa página do site (com os dados de mentira do site-mock.cjs): o tipo,
   o texto (ou o title / aria-label) e onde está. Serve para conferir, página por página, se o app deixou alguma função
   de fora.   node tools/site-cliques.cjs "/u/?id=1"      (CLICAR="seletor; @x,y" antes de listar, como no site-mock) */
const { open } = require("./site-mock.cjs");
const PAGE = process.argv[2] || "/";
open(PAGE, async (page) => {
  for (const sel of (process.env.CLICAR || "").split(";").map((x) => x.trim()).filter(Boolean)) {
    const m = sel.match(/^(@@?)(\d+),(\d+)$/);
    if (m) await page.mouse.click(+m[2], +m[3], { button: m[1] === "@@" ? "right" : "left" });
    else await page.click(sel);
    await page.waitForTimeout(400);
  }
  const out = await page.evaluate(() => {
    const sel = 'a[href], button, input, textarea, select, summary, [role="button"], [role="tab"], [role="option"], [role="menuitem"], [role="radio"], [tabindex]:not([tabindex="-1"]), [onclick], [data-action], [data-tab], label.toggle-row';
    const seen = new Set();
    const rows = [];
    for (const e of document.querySelectorAll(sel)) {
      const r = e.getBoundingClientRect();
      if (r.width < 2 || r.height < 2) continue;
      const cs = getComputedStyle(e);
      if (cs.visibility === "hidden" || cs.display === "none" || +cs.opacity === 0) continue;
      if (r.bottom < 0 || r.right < 0 || r.top > innerHeight || r.left > innerWidth) continue;
      // o elemento de cima naquele ponto tem que ser ele (ou estar dentro dele): o que está coberto não conta
      const top = document.elementFromPoint(Math.min(innerWidth - 1, r.x + r.width / 2), Math.min(innerHeight - 1, r.y + r.height / 2));
      if (top && !e.contains(top) && !top.contains(e)) continue;
      const txt = (e.getAttribute("aria-label") || e.getAttribute("title") || e.innerText || e.value || e.placeholder || "").replace(/\s+/g, " ").trim().slice(0, 60);
      const tag = e.tagName.toLowerCase() + (e.type && e.tagName !== "BUTTON" ? ":" + e.type : "");
      const key = tag + txt + Math.round(r.x) + Math.round(r.y);
      if (seen.has(key)) continue;
      seen.add(key);
      rows.push(`${String(Math.round(r.x)).padStart(4)},${String(Math.round(r.y)).padStart(4)}  ${tag.padEnd(14)} ${txt}`);
    }
    return rows.join("\n");
  });
  console.log(out);
});
