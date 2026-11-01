#!/usr/bin/env node
/* Copia as capas dos jogos do site (/jogos/, desenhadas pelo jogos.js) para assets/jogos/, em camadas.

   Cada capa do site é um SVG 300×200 cujas cores vêm do CSS (`.jg-art-a` e as outras saem do `hue` do jogo) e cujos
   grupos `.jg-fl`, `.jg-fl2` e `.jg-fl3` flutuam por animação de CSS. O app não roda CSS, então este script abre a
   página de verdade (com o site-mock.cjs), lê a cor que o navegador calculou para cada forma e grava:

     <jogo>-base.svg   o que fica parado
     <jogo>-fl.svg     cada grupo que flutua, numa camada própria (o app move a camada inteira)
     <jogo>-fl2.svg
     <jogo>-fl3.svg

   O `<text>` (o "?" do Quem disse isso?) não vai para o arquivo: o leitor de SVG do app não tem a fonte do site.
   Ele aparece na saída para conferir, e o app o escreve com a Patua One (`jogos.rs`).

   node tools/extrair-jogos.cjs                     as capas do tema escuro
   ESQUEMA=light node tools/extrair-jogos.cjs       as do claro, com "-claro" no nome (<jogo>-base-claro.svg)
   (rodar os dois de novo quando o site mudar uma capa) */
const fs = require("fs");
const path = require("path");
const { open } = require("./site-mock.cjs");

const OUT = path.join(__dirname, "..", "assets", "jogos");

open("/jogos/", async (page) => {
  const capas = await page.evaluate(() => {
    /* "rgb(1, 2, 3)" ou "rgba(1, 2, 3, 0.35)" → a cor sem transparência e a transparência à parte, que é o que o
       leitor de SVG entende com certeza */
    const cor = (c) => {
      const m = c.match(/rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?\)/);
      if (!m) return null;
      const hex = "#" + [m[1], m[2], m[3]].map((v) => Math.round(+v).toString(16).padStart(2, "0")).join("");
      return { hex, a: m[4] === undefined ? 1 : +m[4] };
    };
    const out = {};
    for (const card of document.querySelectorAll(".jg-card")) {
      const id = card.dataset.id;
      const camadas = { base: [], fl: [], fl2: [], fl3: [] };
      const textos = [];
      const anda = (el, camada) => {
        for (const c of el.children) {
          const cls = c.getAttribute("class") || "";
          if (c.tagName === "g") {
            const m = cls.match(/jg-(fl[23]?)\b/);
            anda(c, m ? m[1] : camada);
            continue;
          }
          const cs = getComputedStyle(c);
          if (c.tagName === "text") {
            textos.push({ camada, x: c.getAttribute("x"), y: c.getAttribute("y"), tamanho: c.getAttribute("font-size"), cor: cor(cs.fill) });
            continue;
          }
          const n = c.cloneNode(false);
          n.removeAttribute("class");
          for (const [prop, attr] of [["fill", "fill"], ["stroke", "stroke"]]) {
            const v = cs[prop];
            if (!v || v === "none") {
              n.setAttribute(attr, "none");
              continue;
            }
            const k = cor(v);
            if (!k) continue;
            n.setAttribute(attr, k.hex);
            if (k.a < 1) n.setAttribute(attr + "-opacity", String(k.a));
          }
          camadas[camada].push(n.outerHTML);
        }
      };
      anda(card.querySelector(".jg-card__capa svg"), "base");
      out[id] = { camadas, textos };
    }
    return out;
  });

  fs.mkdirSync(OUT, { recursive: true });
  const sufixo = process.env.ESQUEMA === "light" ? "-claro" : "";
  // apaga só as do tema que vai gravar
  for (const f of fs.readdirSync(OUT)) if (f.endsWith(".svg") && f.endsWith("-claro.svg") === !!sufixo) fs.unlinkSync(path.join(OUT, f));
  for (const [id, { camadas, textos }] of Object.entries(capas)) {
    for (const [nome, formas] of Object.entries(camadas)) {
      if (!formas.length) continue;
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 200" width="300" height="200">${formas.join("")}</svg>\n`;
      fs.writeFileSync(path.join(OUT, `${id}-${nome}${sufixo}.svg`), svg);
    }
    console.log(id, Object.entries(camadas).filter(([, f]) => f.length).map(([n, f]) => `${n}:${f.length}`).join(" "), textos.length ? "texto: " + JSON.stringify(textos) : "");
  }
});
