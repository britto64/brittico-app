#!/usr/bin/env node
/* Uma página do site com os dados de mentira do app, para comparar as duas telas lado a lado.

   O site só tem modo de teste na Call (/call/?mock=1), então este script faz o papel dele nas outras: serve a pasta
   `public/` do site em localhost e responde no lugar de api.brittico.xyz com o que está em assets/mock/ (home.json,
   jogos.json), os mesmos arquivos que o app lê. A pessoa logada é o Kennzy, como na Call do app.

   node tools/site-mock.cjs [saida.png] [caminho da página, "/" ou "/jogos/"] [pasta public do site]
   Também é um módulo: `require("./site-mock.cjs").open(caminho, async (page) => ...)` (o extrair-jogos.cjs usa).
   Precisa do playwright-core e do Chromium dele (ver a memória "ambiente-teste-navegador-sem-chrome"). */
const http = require("http");
const fs = require("fs");
const path = require("path");

const OUT = process.argv[2] || "site.png";
const PAGE = process.argv[3] || "/";
const SITE = process.argv[4] || "/mnt/movespeed/claude/1.servidor/1.site_brittico/public";
const PW = process.env.PLAYWRIGHT_CORE || "/mnt/movespeed/claude/9.tiairannice-dash/e2e/node_modules/playwright-core";
const CHROME = process.env.CHROME_PATH || path.join(process.env.HOME, ".cache/ms-playwright/chromium-1234/chrome-linux64/chrome");
const ASSETS = path.join(__dirname, "..", "assets");
const M = JSON.parse(fs.readFileSync(path.join(ASSETS, "mock/home.json"), "utf8"));
const J = JSON.parse(fs.readFileSync(path.join(ASSETS, "mock/jogos.json"), "utf8"));
const LP = JSON.parse(fs.readFileSync(path.join(ASSETS, "mock/livepixel.json"), "utf8"));
const LP_PALETTE = ["#1a1c2c","#5d275d","#b13e53","#ef7d57","#ffcd75","#a7f070","#38b764","#257179","#29366f","#3b5dc9","#41a6f6","#73eff7","#f4f4f4","#94b0c2","#566c86","#333c57"];
const lpStreamer = (id) => {
  const s = LP.streamers.find((x) => x.id === id) || { id, name: id };
  return { id: s.id, username: s.name, display_name: s.name, profile_image: s.avatar ? API_ORIGIN + "/api/feed/images/" + s.avatar : "" };
};
/* As artes do livepixel.json viram a lista de pixels que o WebSocket do jogo manda (`pixel_batch`). */
function lpPixels() {
  const out = [];
  for (const a of LP.artes) a.rows.forEach((row, dy) => [...row].forEach((ch, dx) => {
    if (ch === ".") return;
    out.push({ x: a.x + dx, y: a.y + dy, color: LP_PALETTE[parseInt(ch, 16)], username: a.user, userAvatar: "", userId: "u-" + a.user,
      placedAt: NOW - (dx * 7 + dy * 13) * 60000, streamerId: a.streamer, streamerName: lpStreamer(a.streamer).display_name });
  }));
  return out;
}
const lpChat = (m) => ({ ...m, streamerName: lpStreamer(m.streamerId).display_name, createdAt: NOW });

const NOW = Date.now();
/* A data como o banco grava: UTC, sem fuso, com espaço. */
const at = (min) => new Date(NOW - min * 60000).toISOString().replace("T", " ").replace("Z", "");
const img = (f) => (f ? "mock/" + f : null);

let cid = 1;
function post(p) {
  return {
    id: p.id, username: p.username, channel: p.channel || null, post_type: p.post_type,
    title: p.title || null, message_text: p.message_text || null, timestamp: at(p.ago_min),
    score: p.score, my_vote: 0, comment_count: p.comment_count,
    image_filename: img(p.image), profile_avatar: p.avatar ? API_ORIGIN + "/api/feed/images/" + p.avatar : null,
    profile_id: p.username === M.me.login ? 1 : null,
  };
}
const API_ORIGIN = "https://api.brittico.xyz";

function api(url) {
  const u = new URL(url);
  const p = u.pathname;
  if (p === "/api/me") return { loggedIn: true, user: { ...M.me, avatar: API_ORIGIN + "/api/feed/images/" + M.me.avatar }, isModerator: true, isBroadcaster: true, profileId: 1, perms: [] };
  if (p === "/api/feed") return { posts: Number(u.searchParams.get("offset") || 0) ? [] : M.posts.map(post) };
  let m = p.match(/^\/api\/feed\/(\d+)\/comments$/);
  if (m) {
    const src = M.posts.find((x) => x.id === Number(m[1]));
    return { comments: (src ? src.comments : []).map((c) => ({ id: cid++, author_name: c.author_name, body: c.body, created_at: at(c.ago_min), profile_id: c.author_name === src.username ? 77 : null })) };
  }
  if (p === "/api/bolotas/status") return J.bolotas;
  if (p === "/api/quemdisse/status") return J.quemdisse;
  /* o leaderboard da casa dos jogos (só os nomes) e o do próprio LivePixel (com a contagem) */
  if (p === "/api/livepixel/leaderboard" && aberta.startsWith("/jogos")) return { entries: J.livepixel.entries.map((name) => ({ name })) };
  if (p === "/api/livepixel/leaderboard") return { entries: LP.leaderboard.map((e) => ({ ...lpStreamer(e.id), pixel_count: e.pixel_count })) };
  if (p === "/api/livepixel/leaderboard/artists") return { entries: LP.artists, me: LP.artists_me };
  if (p === "/api/livepixel/user/me") return { streamerId: LP.me.streamer, streamer: lpStreamer(LP.me.streamer),
    cooldown: { pixels_remaining: LP.me.pixels, normal_pixels_remaining: LP.me.pixels, cooldown_ends_at: 0 } };
  if (p === "/api/livepixel/spin-status") return { ok: true, canSpin: true, nextSpinAt: 0, secondsLeft: 0 };
  if (p.startsWith("/api/livepixel/")) return {};
  if (p === "/api/profile/resolve") return { id: 1 };
  if (p === "/api/me/link-options") return { twitch: true, youtube: true };
  if (p === "/api/profile/1") {
    const P = M.profile;
    return {
      profile: { id: 1, displayName: M.me.display_name, avatar: API_ORIGIN + "/api/feed/images/" + M.me.avatar, createdAt: P.since, bio: P.bio, bannerColor: P.banner_color,
        roles: P.roles, claimed: P.claimed, isPublic: true, showMessages: true, showMessagesLimit: 50,
        points: { month: P.points_month, total: P.points_total, batidas: P.points_batidas } },
      isOwner: true, identities: P.identities, stats: P.stats.map((s) => ({ ...s, firstAt: P.since })), totalMessages: P.messages,
      messages: P.chat.map((m, i) => ({ id: 1000 - i, platform: m.platform, username: M.me.login, message: m.message, timestamp: at(m.ago_min) })),
      messagesHidden: false, messagesLimited: false,
    };
  }
  if (p === "/api/profile/1/activity") {
    const days = [];
    let total = 0;
    M.profile.activity.forEach((n, i) => {
      const d = new Date(NOW - i * 86400000);
      const iso = d.getFullYear() + "-" + String(d.getMonth() + 1).padStart(2, "0") + "-" + String(d.getDate()).padStart(2, "0");
      if (n) days.push({ day: iso, count: n });
      total += n;
    });
    return { days, total, hidden: false };
  }
  if (p === "/api/profile/1/posts")
    return { posts: M.my_posts.map((x) => ({ ...x, username: M.me.login, timestamp: at(x.ago_min), image_filename: img(x.image) })), total: M.my_posts_total };
  return {};
}

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png", ".jpg": "image/jpeg", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".ico": "image/x-icon", ".webp": "image/webp", ".json": "application/json" };
const server = http.createServer((req, res) => {
  let f = decodeURIComponent(new URL(req.url, "http://x").pathname);
  if (f.endsWith("/")) f += "index.html";
  const file = path.join(SITE, f);
  fs.readFile(file, (err, buf) => {
    if (err) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { "Content-Type": TYPES[path.extname(file)] || "application/octet-stream" });
    res.end(buf);
  });
});

/* Abre `page` (o caminho) com a API respondida pelos arquivos de mentira e entrega a página do Playwright a `fn`. */
let aberta = PAGE;
function open(pagePath, fn) {
  aberta = pagePath;
  return new Promise((resolve, reject) => {
    server.listen(0, "127.0.0.1", async () => {
      const { chromium } = require(PW);
      const b = await chromium.launch({ executablePath: CHROME });
      try {
        const ctx = await b.newContext({ viewport: { width: 1600, height: 900 }, colorScheme: process.env.ESQUEMA === "light" ? "light" : "dark" });
        await ctx.route(API_ORIGIN + "/**", async (route) => {
          const url = route.request().url();
          const im = url.match(/\/api\/feed\/images\/(.+)$/);
          if (im) return route.fulfill({ path: path.join(ASSETS, decodeURIComponent(im[1])) });
          if (/\/events/.test(url)) return route.fulfill({ status: 204, body: "" });
          route.fulfill({ status: 200, contentType: "application/json", headers: { "Access-Control-Allow-Origin": "http://127.0.0.1:" + server.address().port, "Access-Control-Allow-Credentials": "true" }, body: JSON.stringify(api(url)) });
        });
        /* nenhum WebSocket sai daqui: o do LivePixel ganha os pixels e o chat do livepixel.json, os outros ficam mudos */
        await ctx.routeWebSocket(/.*/, (ws) => {
          if (!/livepixel\/ws/.test(ws.url())) return;
          setTimeout(() => {
            ws.send(JSON.stringify({ type: "pixel_batch", pixels: lpPixels() }));
            ws.send(JSON.stringify({ type: "chat_history", channel: "global", messages: LP.chat.filter((m) => m.channel === "global").map(lpChat) }));
            ws.send(JSON.stringify({ type: "chat_history", channel: "streamer", messages: LP.chat.filter((m) => m.channel !== "global").map(lpChat) }));
          }, 300);
        });
        const page = await ctx.newPage();
        await page.goto("http://" + (process.env.HOST || "127.0.0.1") + ":" + server.address().port + pagePath, { waitUntil: "networkidle" });
        await page.waitForTimeout(1500);
        resolve(await fn(page));
      } catch (e) {
        reject(e);
      } finally {
        await b.close();
        server.close();
      }
    });
  });
}
module.exports = { open };

if (require.main === module) {
  open(PAGE, async (page) => {
    /* CLICAR="seletor; seletor" clica em cada um, na ordem, antes do print (as abas do perfil, por exemplo) */
    for (const sel of (process.env.CLICAR || "").split(";").map((x) => x.trim()).filter(Boolean)) {
      const m = sel.match(/^(@@?)(\d+),(\d+)$/);
      if (m) await page.mouse.click(+m[2], +m[3], { button: m[1] === "@@" ? "right" : "left" });
      else await page.click(sel);
      await page.waitForTimeout(400);
    }
    /* ROLAR=600 desce o conteúdo da página (o `.main`, que é quem rola no site) antes do print */
    if (process.env.ROLAR) {
      await page.evaluate((y) => document.querySelector(".main").scrollTo(0, y), Number(process.env.ROLAR));
      await page.waitForTimeout(300);
    }
    await page.screenshot({ path: OUT });
    /* MEDIR="seletor, seletor" imprime onde cada peça ficou: x, y, largura, altura. */
    if (process.env.MEDIR) {
      const out = await page.evaluate((sels) => sels.map((sel) => [...document.querySelectorAll(sel)].slice(0, 6).map((e) => {
        const r = e.getBoundingClientRect();
        return sel + " " + [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 10) / 10).join(" ");
      }).join("\n")).join("\n"), process.env.MEDIR.split(",").map((s) => s.trim()));
      console.log(out);
    }
    console.log(OUT);
  });
}
