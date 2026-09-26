// ══════════════════════════════════════════════════════════════════
// sw.js — Service worker VEX (etape 5 feuille de route : app mobile/PWA)
//
// Portee volontairement etroite : met en cache UNIQUEMENT les fichiers
// static reellement statiques (CSS, JS partages, icones SVG/PNG) pour
// un chargement plus rapide et un minimum de fonctionnement hors-ligne
// (coquille de page). Ne met JAMAIS en cache :
//   - les pages HTML servies dynamiquement (contiennent nav/theme/i18n
//     et des donnees de session injectees cote serveur a chaque requete)
//   - toute route /api/* (donnees toujours fraiches, jamais de cache
//     sur des fichiers chiffres/messages/etat de session)
// ══════════════════════════════════════════════════════════════════

// v2 : purge l'ancien cache (theme.css sans degrade de nav).
// v3 : theme.css versionne (?v=) + icones .vi a la place des emojis.
// v4 : palette sombre noire.
// v6 : CSS/JS en reseau d'abord.
const CACHE_NAME = 'vex-static-v6';
const STATIC_PREFIXES = ['/static/css/', '/static/img/', '/static/js/', '/static/fa-local.js', '/static/crypto.js'];

self.addEventListener('install', (event) => {
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then(keys =>
      Promise.all(keys.filter(k => k !== CACHE_NAME).map(k => caches.delete(k)))
    ).then(() => self.clients.claim())
  );
});

function estStatique(url) {
  return STATIC_PREFIXES.some(p => url.pathname.startsWith(p));
}

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);
  if (event.request.method !== 'GET' || url.origin !== self.location.origin) return;
  if (!estStatique(url)) return; // laisse passer sans intervention (jamais l'API/HTML)

  // CSS/JS : reseau d'abord (un correctif deploye est visible au premier
  // rechargement ; avant, l'ancienne copie en cache etait servie d'abord et
  // les changements semblaient "pas appliques"). Cache = secours hors-ligne.
  // Images : cache d'abord (elles ne changent pas).
  const reseauDabord = /\.(css|js)$/.test(url.pathname);
  event.respondWith(
    caches.open(CACHE_NAME).then(async (cache) => {
      const cached = await cache.match(event.request);
      const fetchPromise = fetch(event.request).then((resp) => {
        if (resp && resp.ok) cache.put(event.request, resp.clone());
        return resp;
      }).catch(() => cached);
      if (reseauDabord) return fetchPromise;
      return cached || fetchPromise;
    })
  );
});
