// ══════════════════════════════════════════════════════════════════
// fa-local.js — Font Awesome local via /static/img/solid/*.svg
// Remplace tous les <i class="fas fa-xxx"> par le SVG correspondant
// Aucune dépendance externe
// ══════════════════════════════════════════════════════════════════
(function () {
  'use strict';

  const BASE = '/static/img/solid/';
  const cache = {};

  // Récupère un SVG et le met en cache
  async function fetchSvg(name) {
    if (cache[name] !== undefined) return cache[name];
    try {
      const r = await fetch(BASE + name + '.svg');
      if (!r.ok) { cache[name] = null; return null; }
      const text = await r.text();
      // Extrait le contenu interne du SVG (viewBox + path)
      const match = text.match(/<svg([^>]*)>([\s\S]*?)<\/svg>/i);
      if (!match) { cache[name] = null; return null; }
      cache[name] = { attrs: match[1], inner: match[2] };
      return cache[name];
    } catch {
      cache[name] = null;
      return null;
    }
  }

  // Remplace un élément <i class="fas fa-xxx"> par un <svg>
  async function replaceIcon(el) {
    // Trouve le nom de l'icône : fa-gauge → gauge
    const iconClass = Array.from(el.classList)
      .find(c => c.startsWith('fa-') && c !== 'fas' && c !== 'far' && c !== 'fab');
    if (!iconClass) return;

    const name = iconClass.replace(/^fa-/, '');
    const data = await fetchSvg(name);
    if (!data) return;

    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');

    // Copie les attributs du SVG original (viewBox, etc.)
    const tmp = document.createElement('div');
    tmp.innerHTML = `<svg${data.attrs}></svg>`;
    const src = tmp.querySelector('svg');
    if (src) {
      for (const attr of src.attributes) {
        svg.setAttribute(attr.name, attr.value);
      }
    }

    // Dimensions et style cohérents avec Font Awesome
    svg.setAttribute('width', '1em');
    svg.setAttribute('height', '1em');
    svg.setAttribute('fill', 'currentColor');
    svg.setAttribute('aria-hidden', 'true');
    svg.style.display = 'inline-block';
    svg.style.verticalAlign = '-0.125em';
    svg.style.flexShrink = '0';

    // Transfère les classes (pour couleur, taille CSS custom)
    svg.setAttribute('class', el.getAttribute('class') || '');
    svg.innerHTML = data.inner;

    el.parentNode.replaceChild(svg, el);
  }

  // Traite tous les <i class="fas ..."> du nœud donné
  function processNode(root) {
    const els = (root.querySelectorAll
      ? root.querySelectorAll('i[class*="fa-"]')
      : []);
    els.forEach(replaceIcon);
    // Si root lui-même est un <i>
    if (root.tagName === 'I' && root.className && root.className.includes('fa-')) {
      replaceIcon(root);
    }
  }

  // Traite la page entière au chargement
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => processNode(document));
  } else {
    processNode(document);
  }

  // MutationObserver pour les éléments ajoutés dynamiquement (SPA admin)
  const obs = new MutationObserver(mutations => {
    for (const m of mutations) {
      for (const node of m.addedNodes) {
        if (node.nodeType === 1) processNode(node);
      }
    }
  });
  obs.observe(document.body, { childList: true, subtree: true });
})();