// Enregistrement du service worker PWA -- inclus sur toutes les pages VEX.
// Echoue silencieusement si non supporte (navigateur ancien, contexte non
// securise) : la page fonctionne normalement sans PWA dans ce cas.
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/static/sw.js', { scope: '/' }).catch(() => {});
  });
}
