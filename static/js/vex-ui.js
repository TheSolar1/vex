// ══════════════════════════════════════════════════════════════════
// vex-ui.js — comportements d'interface communs à toutes les pages
// (chargé par la barre de navigation, function::build_nav_html).
//   1. Thème « Automatique » : data-theme="auto" suit le réglage clair /
//      sombre de l'appareil, en direct.
//   2. Raccourcis clavier : Ctrl+K (ou /) = recherche globale,
//      ? = aide des raccourcis.
// ══════════════════════════════════════════════════════════════════
(function () {
  'use strict';
  if (window.__vexUi) return;
  window.__vexUi = true;

  // ── 1. Thème automatique ─────────────────────────────────────────
  var html = document.documentElement;
  var mq = window.matchMedia ? window.matchMedia('(prefers-color-scheme: dark)') : null;
  var modeAuto = false;
  function resoudre() { return mq && mq.matches ? 'dark' : 'light'; }
  function appliquer() {
    if (html.getAttribute('data-theme') === 'auto') modeAuto = true;
    if (modeAuto && html.getAttribute('data-theme') !== resoudre()) html.setAttribute('data-theme', resoudre());
  }
  appliquer();
  // Les pages qui posent le thème en JS (fchier, mess, sitec...) repassent
  // par ici : un "auto" posé plus tard est aussi résolu. Un thème explicite
  // (light/dark) posé ensuite désactive le mode auto.
  new MutationObserver(function () {
    var t = html.getAttribute('data-theme');
    if (t === 'auto') { modeAuto = true; appliquer(); }
    else if (t !== resoudre()) modeAuto = false;
  }).observe(html, { attributes: true, attributeFilter: ['data-theme'] });
  if (mq) {
    var suivre = function () { if (modeAuto) html.setAttribute('data-theme', resoudre()); };
    if (mq.addEventListener) mq.addEventListener('change', suivre); else if (mq.addListener) mq.addListener(suivre);
  }
  // Certaines pages posent data-theme sur <body> (dashboard).
  function corpsAuto() {
    if (document.body && document.body.getAttribute('data-theme') === 'auto') {
      document.body.setAttribute('data-theme', resoudre());
      if (mq) {
        var f = function () { document.body.setAttribute('data-theme', resoudre()); };
        if (mq.addEventListener) mq.addEventListener('change', f); else if (mq.addListener) mq.addListener(f);
      }
    }
  }
  if (document.body) corpsAuto(); else document.addEventListener('DOMContentLoaded', corpsAuto);

  // ── 2. Raccourcis clavier ────────────────────────────────────────
  function champSaisie(el) {
    if (!el) return false;
    var tag = el.tagName;
    return el.isContentEditable || tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
  }
  function allerRecherche() {
    var q = document.getElementById('q');
    if (q && location.pathname.indexOf('/recherche') === 0) { q.focus(); q.select(); return; }
    location.href = '/recherche/?focus=1';
  }
  function aide() {
    var id = 'vex-raccourcis';
    var ex = document.getElementById(id);
    if (ex) { ex.remove(); return; }
    var d = document.createElement('div');
    d.id = id;
    d.setAttribute('role', 'dialog');
    d.setAttribute('aria-label', 'Raccourcis clavier');
    d.style.cssText = 'position:fixed;inset:0;z-index:5000;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,.45)';
    d.innerHTML = '<div style="background:var(--surface,#fff);color:var(--text,#111);border:1px solid var(--panel-border,#ddd);border-radius:14px;padding:22px 26px;min-width:280px;max-width:90vw;box-shadow:0 10px 40px rgba(0,0,0,.3);font-family:system-ui,sans-serif">'
      + '<h2 style="margin:0 0 14px;font-size:1.05rem">Raccourcis clavier</h2>'
      + '<table style="border-collapse:collapse;font-size:.9rem">'
      + ligne('Ctrl + K  ou  /', 'Recherche globale')
      + ligne('?', 'Afficher / masquer cette aide')
      + ligne('Échap', 'Fermer une fenêtre ou un menu')
      + ligne('← →', 'Fichier précédent / suivant (aperçu ExoDrive)')
      + '</table><p style="margin:14px 0 0;font-size:.78rem;opacity:.7">Échap ou clic pour fermer</p></div>';
    d.addEventListener('click', function () { d.remove(); });
    document.body.appendChild(d);
  }
  function ligne(k, v) {
    return '<tr><td style="padding:5px 16px 5px 0"><kbd style="font-family:monospace;background:var(--surface2,#f4f4f4);border:1px solid var(--border,#ccc);border-radius:5px;padding:2px 7px">' + k + '</kbd></td><td style="padding:5px 0">' + v + '</td></tr>';
  }
  document.addEventListener('keydown', function (e) {
    if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key === 'k' || e.key === 'K')) {
      e.preventDefault(); allerRecherche(); return;
    }
    if (champSaisie(e.target) || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === '/') { e.preventDefault(); allerRecherche(); }
    else if (e.key === '?') { e.preventDefault(); aide(); }
    else if (e.key === 'Escape') { var a = document.getElementById('vex-raccourcis'); if (a) a.remove(); }
  });
  // Arrivée sur la recherche via le raccourci : focus direct du champ.
  if (location.pathname.indexOf('/recherche') === 0 && /[?&]focus=1/.test(location.search)) {
    var focusQ = function () { var q = document.getElementById('q'); if (q) q.focus(); };
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', focusQ); else focusQ();
  }
})();
