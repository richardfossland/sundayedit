/* SundayEdit landing — small, dependency-free interactions.
 *
 * The centrepiece is the interactive "review threshold": each word carries a
 * confidence score; words scoring below the threshold light up by tier, exactly
 * as they do in the app. Everything else is progressive enhancement — the page
 * is fully readable with JS off. */

(function () {
  "use strict";

  /* ── Interactive confidence demo ───────────────────────────────────────── */
  // [word, confidence]. The jargon/names are deliberately low — the same kind
  // of words the glossary feature exists to rescue.
  const WORDS = [
    ["The", 99],
    ["keynote", 96],
    ["on", 99],
    ["kerygma", 38],
    ["and", 98],
    ["soteriology", 44],
    ["drew", 88],
    ["on", 99],
    ["Dr.", 91],
    ["Bultmann's", 52],
    ["reading", 94],
    ["of", 99],
    ["the", 99],
    ["gospel", 82],
    ["of", 99],
    ["John.", 90],
  ];

  function tierFor(conf) {
    if (conf < 50) return "t4";
    if (conf < 70) return "t3";
    return "t2"; // 70–84 band
  }

  const line = document.getElementById("captionLine");
  const thr = document.getElementById("thr");
  const thrVal = document.getElementById("thrVal");
  const flagCount = document.getElementById("flagCount");

  if (line && thr) {
    const spans = WORDS.map(([text, conf]) => {
      const s = document.createElement("span");
      s.className = "w";
      s.textContent = text;
      s.dataset.conf = String(conf);
      return s;
    });
    // Re-join with spaces so the sentence reads naturally.
    spans.forEach((s, i) => {
      line.appendChild(s);
      if (i < spans.length - 1) line.appendChild(document.createTextNode(" "));
    });

    const render = () => {
      const t = Number(thr.value);
      thrVal.textContent = String(t);
      let flagged = 0;
      for (const s of spans) {
        const conf = Number(s.dataset.conf);
        s.classList.remove("flag", "t2", "t3", "t4");
        if (conf < t) {
          flagged++;
          s.classList.add("flag", tierFor(conf));
        }
      }
      flagCount.textContent = String(flagged);
    };

    thr.addEventListener("input", render);
    render();
  }

  /* ── Faux waveform bars ────────────────────────────────────────────────── */
  const wave = document.querySelector(".wave");
  if (wave) {
    const N = 64;
    // Deterministic pseudo-random so the shape is stable across reloads.
    let seed = 7;
    const rnd = () => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed / 0x7fffffff;
    };
    for (let i = 0; i < N; i++) {
      const bar = document.createElement("i");
      const env = Math.sin((i / N) * Math.PI); // taper the ends
      const h = 8 + Math.round((0.25 + 0.75 * rnd()) * env * 100);
      bar.style.height = Math.min(100, h) + "%";
      bar.style.animationDelay = i * 28 + "ms";
      wave.appendChild(bar);
    }
  }

  /* ── Scroll reveal ─────────────────────────────────────────────────────── */
  const reveals = document.querySelectorAll(".reveal");
  if ("IntersectionObserver" in window && reveals.length) {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            e.target.classList.add("in");
            io.unobserve(e.target);
          }
        }
      },
      { threshold: 0.12, rootMargin: "0px 0px -40px 0px" },
    );
    reveals.forEach((el) => io.observe(el));
  } else {
    reveals.forEach((el) => el.classList.add("in"));
  }

  /* ── Nav: shadow on scroll + mobile toggle ─────────────────────────────── */
  const nav = document.querySelector(".nav");
  const onScroll = () =>
    nav && nav.classList.toggle("scrolled", window.scrollY > 8);
  onScroll();
  window.addEventListener("scroll", onScroll, { passive: true });

  const toggle = document.getElementById("navToggle");
  const links = document.querySelector(".nav-links");
  if (toggle && links) {
    toggle.addEventListener("click", () => {
      const open = links.classList.toggle("show");
      toggle.setAttribute("aria-expanded", String(open));
    });
    links.querySelectorAll("a").forEach((a) =>
      a.addEventListener("click", () => {
        links.classList.remove("show");
        toggle.setAttribute("aria-expanded", "false");
      }),
    );
  }

  /* ── Year ──────────────────────────────────────────────────────────────── */
  const year = document.getElementById("year");
  if (year) year.textContent = String(new Date().getFullYear());
})();
