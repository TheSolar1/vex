// fp.js — empreinte d'appareil VEX (securite : reperer quel PC/telephone
// se connecte, meme si l'IP change). Calcule un hash SHA-256 a partir de
// caracteristiques stables du navigateur/materiel, puis l'envoie a
// /api/empreinte. Aucune donnee saisie par l'utilisateur n'est lue.
// Declare dans la Politique de Confidentialite (/constiontu.html).
(function () {
  'use strict';

  function canvasFp() {
    try {
      var c = document.createElement('canvas');
      c.width = 240; c.height = 60;
      var x = c.getContext('2d');
      x.textBaseline = 'top';
      x.font = '16px Arial';
      x.fillStyle = '#f60';
      x.fillRect(100, 1, 62, 20);
      x.fillStyle = '#069';
      x.fillText('VEX empreinte ❤ 1234', 2, 15);
      x.fillStyle = 'rgba(102,204,0,0.7)';
      x.fillText('VEX empreinte ❤ 1234', 4, 17);
      return c.toDataURL();
    } catch (e) { return 'x'; }
  }

  function webglFp() {
    try {
      var gl = document.createElement('canvas').getContext('webgl');
      if (!gl) return 'x';
      var ext = gl.getExtension('WEBGL_debug_renderer_info');
      return ext
        ? gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) + '|' + gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)
        : gl.getParameter(gl.VENDOR) + '|' + gl.getParameter(gl.RENDERER);
    } catch (e) { return 'x'; }
  }

  // Polices installees : largeur d'un texte avec chaque police, comparee
  // aux polices generiques de repli.
  function policesFp() {
    var liste = ['Arial', 'Calibri', 'Cambria', 'Consolas', 'Courier New', 'Georgia',
      'Helvetica', 'Segoe UI', 'Tahoma', 'Times New Roman', 'Trebuchet MS', 'Verdana',
      'Ubuntu', 'DejaVu Sans', 'Roboto', 'Menlo', 'Comic Sans MS'];
    var generiques = ['monospace', 'serif', 'sans-serif'];
    try {
      var x = document.createElement('canvas').getContext('2d');
      var txt = 'mmmmmmmmmmlli10OO';
      var base = {};
      generiques.forEach(function (g) {
        x.font = '72px ' + g; base[g] = x.measureText(txt).width;
      });
      return liste.filter(function (p) {
        return generiques.some(function (g) {
          x.font = '72px "' + p + '",' + g;
          return x.measureText(txt).width !== base[g];
        });
      }).join(',');
    } catch (e) { return 'x'; }
  }

  function hex(buf) {
    return Array.prototype.map.call(new Uint8Array(buf), function (b) {
      return ('0' + b.toString(16)).slice(-2);
    }).join('');
  }

  function envoyer() {
    if (!window.crypto || !crypto.subtle || !window.TextEncoder) return;
    var n = navigator, s = screen;
    var c = {
      ua: n.userAgent,
      langues: (n.languages || [n.language]).join(','),
      plateforme: n.platform || '',
      coeurs: n.hardwareConcurrency || 0,
      memoire: n.deviceMemory || 0,
      tactile: n.maxTouchPoints || 0,
      ecran: s.width + 'x' + s.height + 'x' + s.colorDepth + '@' + (window.devicePixelRatio || 1),
      fuseau: (Intl.DateTimeFormat().resolvedOptions().timeZone || '') + '|' + new Date().getTimezoneOffset(),
      webgl: webglFp(),
      polices: policesFp(),
      canvas: canvasFp()
    };
    var brut = Object.keys(c).map(function (k) { return k + '=' + c[k]; }).join(';');
    crypto.subtle.digest('SHA-256', new TextEncoder().encode(brut)).then(function (h) {
      var charge = JSON.stringify({
        h: hex(h),
        page: location.pathname,
        // Resume lisible pour l'admin (le canvas ne sert qu'au hash)
        d: [c.plateforme, c.ecran, c.coeurs + ' coeurs', c.memoire + ' Go', c.fuseau, c.webgl].join(' · ')
      });
      if (n.sendBeacon) {
        n.sendBeacon('/api/empreinte', charge);
      } else {
        fetch('/api/empreinte', { method: 'POST', body: charge, keepalive: true, credentials: 'same-origin' });
      }
    }).catch(function () {});
  }

  if (document.readyState === 'complete') envoyer();
  else window.addEventListener('load', envoyer);
})();
