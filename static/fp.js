// fp.js — empreinte d'appareil VEX, v2 (securite : reperer quel PC ou
// telephone se connecte, meme si l'IP change).
//
// 20 composants, chacun hashe SEPAREMENT (8 hex) : le serveur compare
// les composants un par un et calcule un pourcentage de correspondance
// (voir src/empreinte.rs) -- une mise a jour du navigateur ou un
// changement d'ecran ne fait donc plus passer un appareil connu pour un
// nouveau. Les numeros de version sont volontairement exclus.
// L'ORDRE des composants doit rester celui de COMPOSANTS dans empreinte.rs.
//
// Aucune donnee saisie par l'utilisateur n'est lue. Declare dans la
// Politique de Confidentialite (/constiontu.html).
(function () {
  'use strict';

  var n = navigator, s = screen;

  function essai(f, defaut) {
    try { var v = f(); return v === undefined || v === null ? defaut : v; } catch (e) { return defaut; }
  }

  // ── 1-2. Navigateur et systeme SANS numero de version ─────────────
  function navigateur() {
    var ua = n.userAgent;
    if (/FxiOS|Firefox\//.test(ua)) return 'Firefox';
    if (/EdgA?\/|Edg\//.test(ua)) return 'Edge';
    if (/OPR\/|Opera/.test(ua)) return 'Opera';
    if (/SamsungBrowser/.test(ua)) return 'Samsung';
    if (/CriOS|Chrome\//.test(ua)) return 'Chrome';
    if (/Safari\//.test(ua)) return 'Safari';
    return 'Autre';
  }
  function systeme() {
    var ua = n.userAgent;
    if (/iPhone/.test(ua)) return 'iPhone';
    if (/iPad/.test(ua)) return 'iPad';
    if (/Android/.test(ua)) return 'Android';
    if (/Windows/.test(ua)) return 'Windows';
    if (/Mac OS X|Macintosh/.test(ua)) return 'Mac';
    if (/CrOS/.test(ua)) return 'ChromeOS';
    if (/Linux/.test(ua)) return 'Linux';
    return 'Autre';
  }

  // ── 12. Parametres WebGL (limites materielles de la carte graphique) ─
  function webgl() {
    return essai(function () {
      var gl = document.createElement('canvas').getContext('webgl');
      if (!gl) return ['x', 'x'];
      var ext = gl.getExtension('WEBGL_debug_renderer_info');
      var carte = ext
        ? gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) + '|' + gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)
        : gl.getParameter(gl.VENDOR) + '|' + gl.getParameter(gl.RENDERER);
      var params = [
        gl.getParameter(gl.MAX_TEXTURE_SIZE),
        gl.getParameter(gl.MAX_RENDERBUFFER_SIZE),
        Array.prototype.join.call(gl.getParameter(gl.MAX_VIEWPORT_DIMS), 'x'),
        gl.getParameter(gl.MAX_VERTEX_ATTRIBS),
        gl.getParameter(gl.MAX_TEXTURE_IMAGE_UNITS),
        (gl.getSupportedExtensions() || []).sort().join(',')
      ].join(';');
      return [carte, params];
    }, ['x', 'x']);
  }

  // ── 13. Polices installees (largeur mesuree vs polices generiques) ──
  function polices() {
    var liste = ['Arial', 'Calibri', 'Cambria', 'Consolas', 'Courier New', 'Georgia',
      'Helvetica', 'Segoe UI', 'Tahoma', 'Times New Roman', 'Trebuchet MS', 'Verdana',
      'Ubuntu', 'DejaVu Sans', 'Roboto', 'Menlo', 'Comic Sans MS', 'Impact',
      'Lucida Console', 'Palatino Linotype', 'Franklin Gothic Medium', 'Candara'];
    var gen = ['monospace', 'serif', 'sans-serif'];
    return essai(function () {
      var x = document.createElement('canvas').getContext('2d');
      var txt = 'mmmmmmmmmmlli10OO';
      var base = {};
      gen.forEach(function (g) { x.font = '72px ' + g; base[g] = x.measureText(txt).width; });
      return liste.filter(function (p) {
        return gen.some(function (g) {
          x.font = '72px "' + p + '",' + g;
          return x.measureText(txt).width !== base[g];
        });
      }).join(',');
    }, 'x');
  }

  // ── 14. Canvas : texte + formes (anti-crenelage, moteur de rendu) ───
  function canvasTexte() {
    return essai(function () {
      var c = document.createElement('canvas');
      c.width = 240; c.height = 60;
      var x = c.getContext('2d');
      x.textBaseline = 'top';
      x.font = '16px Arial';
      x.fillStyle = '#f60';
      x.fillRect(100, 1, 62, 20);
      x.fillStyle = '#069';
      x.fillText('VEX empreinte 1234 <canvas>', 2, 15);
      x.fillStyle = 'rgba(102,204,0,0.7)';
      x.fillText('VEX empreinte 1234 <canvas>', 4, 17);
      x.globalCompositeOperation = 'multiply';
      x.fillStyle = 'rgb(255,0,255)';
      x.beginPath(); x.arc(50, 50, 40, 0, Math.PI * 2); x.fill();
      return c.toDataURL();
    }, 'x');
  }

  // ── 15. Rendu d'emojis : chaque systeme (et version de police emoji)
  // dessine les emojis differemment -- tres discriminant.
  function canvasEmoji() {
    return essai(function () {
      var emojis = ['😀', '👍🏽', '👨‍👩‍👧‍👦', '🏳️‍🌈', '🦊', '🍕', '🇫🇷', '❤️', '🧑‍💻', '🫠'];
      var c = document.createElement('canvas');
      c.width = 400; c.height = 50;
      var x = c.getContext('2d');
      x.textBaseline = 'top';
      x.font = '32px sans-serif';
      var largeurs = emojis.map(function (e, i) {
        x.fillText(e, i * 40, 5);
        return Math.round(x.measureText(e).width * 100);
      });
      return c.toDataURL() + '|' + largeurs.join(',');
    }, 'x');
  }

  // ── 16. Audio : traitement d'un signal (dependant du CPU/navigateur) ─
  function audio() {
    return new Promise(function (ok) {
      try {
        var AC = window.OfflineAudioContext || window.webkitOfflineAudioContext;
        if (!AC) return ok('x');
        var ctx = new AC(1, 5000, 44100);
        var osc = ctx.createOscillator();
        osc.type = 'triangle';
        osc.frequency.value = 10000;
        var comp = ctx.createDynamicsCompressor();
        comp.threshold.value = -50; comp.knee.value = 40; comp.ratio.value = 12;
        comp.attack.value = 0; comp.release.value = 0.25;
        osc.connect(comp); comp.connect(ctx.destination);
        osc.start(0);
        var fini = false;
        setTimeout(function () { if (!fini) { fini = true; ok('timeout'); } }, 1500);
        ctx.oncomplete = function (ev) {
          if (fini) return;
          fini = true;
          var d = ev.renderedBuffer.getChannelData(0), somme = 0;
          for (var i = 4500; i < 5000; i++) somme += Math.abs(d[i]);
          ok(somme.toFixed(6));
        };
        ctx.startRendering();
      } catch (e) { ok('x'); }
    });
  }

  // ── 17. Precision des fonctions mathematiques (moteur JS / CPU) ─────
  function maths() {
    return essai(function () {
      return [Math.tan(-1e300), Math.acosh(1e300), Math.expm1(1), Math.sinh(1),
        Math.log1p(10), Math.cbrt(100), Math.atanh(0.5), Math.cos(21 * Math.LN2),
        Math.pow(Math.PI, -100)].join(',');
    }, 'x');
  }

  // ── 18. Caracteristiques d'affichage (media queries stables) ────────
  function media() {
    var q = function (m) { return essai(function () { return matchMedia(m).matches ? 1 : 0; }, 'x'); };
    return [
      q('(color-gamut: rec2020)'), q('(color-gamut: p3)'), q('(color-gamut: srgb)'),
      q('(pointer: fine)'), q('(pointer: coarse)'), q('(hover: hover)'),
      q('(any-pointer: coarse)'), q('(forced-colors: active)'),
      q('(prefers-reduced-motion: reduce)'), q('(dynamic-range: high)'),
      q('(inverted-colors: inverted)'), q('(monochrome)')
    ].join('');
  }

  // ── 19. Stockage et reglages de confidentialite ─────────────────────
  function stockage() {
    return [
      n.cookieEnabled ? 1 : 0,
      essai(function () { return window.localStorage ? 1 : 0; }, 0),
      essai(function () { return window.sessionStorage ? 1 : 0; }, 0),
      window.indexedDB ? 1 : 0,
      n.doNotTrack || window.doNotTrack || '-',
      n.globalPrivacyControl ? 1 : 0
    ].join(',');
  }

  // ── 20. Plugins / lecteur PDF ───────────────────────────────────────
  function plugins() {
    return essai(function () {
      var p = Array.prototype.map.call(n.plugins || [], function (x) { return x.name; });
      return (n.pdfViewerEnabled ? 'pdf' : 'nopdf') + '|' + p.sort().join(',');
    }, 'x');
  }

  function sha(texte) {
    return crypto.subtle.digest('SHA-256', new TextEncoder().encode(texte)).then(function (h) {
      return Array.prototype.map.call(new Uint8Array(h), function (b) {
        return ('0' + b.toString(16)).slice(-2);
      }).join('');
    });
  }

  function envoyer() {
    if (!window.crypto || !crypto.subtle || !window.TextEncoder || !window.Promise) return;

    // Protection anti-empreinte (Brave, Firefox strict, extensions) : le
    // canvas change a chaque lecture. On le signale et on neutralise ces
    // composants pour que le reste reste comparable.
    var t1 = canvasTexte(), t2 = canvasTexte();
    var e1 = canvasEmoji(), e2 = canvasEmoji();
    var aleatoire = t1 !== t2 || e1 !== e2;
    var gl = webgl();

    audio().then(function (son) {
      var c = [
        navigateur(),                                                     // 1
        systeme(),                                                        // 2
        (n.languages || [n.language]).join(','),                          // 3
        n.platform || '',                                                 // 4
        n.hardwareConcurrency || 0,                                       // 5
        n.deviceMemory || 0,                                              // 6
        n.maxTouchPoints || 0,                                            // 7
        s.width + 'x' + s.height,                                         // 8
        s.colorDepth + '@' + (window.devicePixelRatio || 1),              // 9
        essai(function () { return Intl.DateTimeFormat().resolvedOptions().timeZone; }, ''), // 10
        gl[0],                                                            // 11
        gl[1],                                                            // 12
        polices(),                                                        // 13
        aleatoire ? 'aleatoire' : t1,                                     // 14
        aleatoire ? 'aleatoire' : e1,                                     // 15
        son,                                                              // 16
        maths(),                                                          // 17
        media(),                                                          // 18
        stockage(),                                                       // 19
        plugins()                                                         // 20
      ].map(String);

      return Promise.all(c.map(sha)).then(function (hashes) {
        var courts = hashes.map(function (h) { return h.slice(0, 8); });
        return sha(courts.join(',')).then(function (global) {
          var charge = JSON.stringify({
            v: 2,
            h: global,
            c: courts,
            page: location.pathname,
            // Resume lisible pour l'admin
            d: [c[1] + ' · ' + c[0], c[7] + ' ' + c[8], c[4] + ' coeurs',
                (c[5] !== '0' ? c[5] + ' Go' : ''), c[9], c[10].split('|').pop(),
                aleatoire ? '⚠ anti-empreinte' : ''].filter(Boolean).join(' · ')
          });
          if (n.sendBeacon) n.sendBeacon('/api/empreinte', charge);
          else fetch('/api/empreinte', { method: 'POST', body: charge, keepalive: true, credentials: 'same-origin' });
        });
      });
    }).catch(function () {});
  }

  if (document.readyState === 'complete') envoyer();
  else window.addEventListener('load', envoyer);
})();
