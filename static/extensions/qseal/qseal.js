/* ══════════════════════════════════════════════════════════════════
   qseal.js — interface QSeal pour VEX.

   Toute la cryptographie tourne ici, dans le navigateur (qseal-core.js).
   Le serveur ne sert qu'a ranger les fichiers de cles dans l'explorateur
   de fichiers VEX : il recoit du base64 opaque, jamais de secret en clair
   des lors qu'une phrase secrete est utilisee.
   ══════════════════════════════════════════════════════════════════ */
(() => {
"use strict";

const Q = globalThis.QSeal;
const API = "/api/ext/qseal";
const CLE_CONTACTS = "qseal-vex-contacts";

/** Identites dechiffrees, en memoire uniquement (jamais persistees ici). */
const ouvertes = new Map();   // keyId -> identite complete
let listeCles = [];           // metadonnees venant du serveur
let contacts = [];            // {nom, keyId, kemPublicKey, dsaPublicKey} en base64

// ── Petits utilitaires ────────────────────────────────────────────
const $ = (id) => document.getElementById(id);
const esc = (t) => String(t).replace(/&/g, "&amp;").replace(/</g, "&lt;")
  .replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function message(texte, erreur = false) {
  const el = $("msg");
  el.textContent = texte;
  el.className = "msg on " + (erreur ? "ko" : "ok");
  clearTimeout(message._t);
  message._t = setTimeout(() => { el.className = "msg"; }, 6000);
}

async function poster(route, donnees) {
  const corps = Object.entries(donnees)
    .map(([k, v]) => encodeURIComponent(k) + "=" + encodeURIComponent(v)).join("&");
  const r = await fetch(API + route, {
    method: "POST", credentials: "include",
    headers: { "Content-Type": "application/x-www-form-urlencoded" }, body: corps,
  });
  return r.json();
}
async function lire(route) {
  const r = await fetch(API + route, { credentials: "include" });
  return r.json();
}

// ── Serialisation d'une identite ──────────────────────────────────
function identiteVersJson(id) {
  return JSON.stringify({
    v: 1, keyId: id.keyId,
    kemPublicKey: Q.bytesToB64(id.kemPublicKey),
    kemSecretKey: Q.bytesToB64(id.kemSecretKey),
    dsaPublicKey: Q.bytesToB64(id.dsaPublicKey),
    dsaSecretKey: Q.bytesToB64(id.dsaSecretKey),
  });
}
function jsonVersIdentite(txt) {
  const o = JSON.parse(txt);
  return {
    keyId: o.keyId,
    kemPublicKey: Q.b64ToBytes(o.kemPublicKey),
    kemSecretKey: Q.b64ToBytes(o.kemSecretKey),
    dsaPublicKey: Q.b64ToBytes(o.dsaPublicKey),
    dsaSecretKey: Q.b64ToBytes(o.dsaSecretKey),
  };
}
/** Le fichier stocke est soit du JSON clair, soit un bloc BACKUP chiffre. */
function enveloppe(contenuTexte, protege) {
  return btoa(unescape(encodeURIComponent(JSON.stringify({
    protege, charge: contenuTexte,
  }))));
}
function ouvreEnveloppe(b64) {
  return JSON.parse(decodeURIComponent(escape(atob(b64))));
}

// ── Cles ──────────────────────────────────────────────────────────
async function chargerCles() {
  const r = await lire("/keys");
  if (!r.success) { message(r.error || "Chargement impossible.", true); return; }
  listeCles = r.data || [];
  rendreCles();
  rendreSelecteurs();
}

function rendreCles() {
  const box = $("liste-cles");
  if (!listeCles.length) {
    box.innerHTML = `<div class="vide">Aucune cle. Generez une identite ci-dessus pour commencer.</div>`;
    return;
  }
  box.innerHTML = listeCles.map((c) => {
    const nom = c.nom.replace(/\.qsealkey$/, "");
    const ouverte = [...ouvertes.values()].find((i) => i._fichier === c.id);
    const etat = ouverte
      ? `<span class="puce p-ok">deverrouillee</span>`
      : `<span class="puce p-neutre">verrouillee</span>`;
    const idCourt = ouverte ? `<div class="cle-id">${esc(ouverte.keyId)}</div>` : "";
    return `<div class="cle">
      <div>
        <div class="cle-nom">${esc(nom)}</div>
        <div class="cle-id">${esc(c.nom)} — ${c.taille} o — ${esc(c.date || "")}</div>
        ${idCourt}
      </div>
      <div class="cle-actions">
        ${etat}
        ${ouverte
          ? `<button class="btn btn-s" data-pub="${c.id}">Cle publique</button>
             <button class="btn btn-s" data-sauv="${c.id}">Sauvegarde</button>`
          : `<button class="btn btn-s" data-ouvrir="${c.id}">Deverrouiller</button>`}
        <button class="btn btn-d" data-suppr="${c.id}">Supprimer</button>
      </div>
    </div>`;
  }).join("");
}

async function ouvrirCle(idFichier) {
  const r = await lire("/keys/get?id=" + encodeURIComponent(idFichier));
  if (!r.success) { message(r.error || "Cle illisible.", true); return null; }
  let env;
  try { env = ouvreEnveloppe(r.data.contenu); }
  catch { message("Fichier de cle illisible ou corrompu.", true); return null; }

  let json;
  if (env.protege) {
    const pass = prompt("Phrase secrete de la cle « " + r.data.nom.replace(/\.qsealkey$/, "") + " » :");
    if (pass === null) return null;
    try {
      const dec = await Q.decryptBackup(pass, env.charge);
      json = typeof dec === "string" ? dec : JSON.stringify(dec);
    } catch (e) { message("Phrase secrete incorrecte.", true); return null; }
  } else {
    json = env.charge;
  }

  try {
    const identite = jsonVersIdentite(json);
    identite._fichier = idFichier;
    ouvertes.set(identite.keyId, identite);
    rendreCles(); rendreSelecteurs();
    message("Cle deverrouillee pour cette session.");
    return identite;
  } catch { message("Contenu de cle invalide.", true); return null; }
}

async function genererIdentite() {
  const nom = $("nouv-nom").value.trim();
  if (!nom) { message("Donnez un nom a la cle.", true); return; }
  const pass = $("nouv-pass").value;
  const btn = $("btn-gen");
  btn.disabled = true; btn.innerHTML = '<span class="spin"></span> Generation…';
  try {
    const id = await Q.generateIdentity();
    const json = identiteVersJson(id);
    const charge = pass ? await Q.encryptBackup(pass, json) : json;
    const r = await poster("/keys/save", { nom, contenu: enveloppe(charge, !!pass) });
    if (!r.success) { message(r.error || "Enregistrement impossible.", true); return; }
    id._fichier = r.data.id;
    ouvertes.set(id.keyId, id);
    $("nouv-nom").value = ""; $("nouv-pass").value = "";
    message(r.message + (pass ? "" : " Attention : cle non protegee par phrase secrete."), !pass);
    await chargerCles();
  } catch (e) {
    message("Echec de la generation : " + e.message, true);
  } finally {
    btn.disabled = false; btn.textContent = "Generer une identite";
  }
}

async function importerCle() {
  const nom = $("imp-nom").value.trim();
  const bloc = $("imp-bloc").value.trim();
  const pass = $("imp-pass").value;
  if (!nom || !bloc) { message("Nom et bloc de sauvegarde requis.", true); return; }
  try {
    const dec = await Q.decryptBackup(pass, bloc);
    const txt = typeof dec === "string" ? dec : JSON.stringify(dec);
    // On revalide avant d'enregistrer : pas de fichier de cle inutilisable.
    jsonVersIdentite(typeof dec === "string" ? txt : extraireIdentite(dec));
    const charge = pass ? await Q.encryptBackup(pass, txt) : txt;
    const r = await poster("/keys/save", { nom, contenu: enveloppe(charge, !!pass) });
    message(r.success ? r.message : (r.error || "Import impossible."), !r.success);
    if (r.success) { $("imp-bloc").value = ""; $("imp-nom").value = ""; $("imp-pass").value = ""; await chargerCles(); }
  } catch (e) {
    message("Import impossible : phrase secrete incorrecte ou bloc invalide.", true);
  }
}
/** Une sauvegarde QSeal peut contenir une liste d'identites. */
function extraireIdentite(obj) {
  if (Array.isArray(obj)) return JSON.stringify(obj[0]);
  if (obj && obj.identities) return JSON.stringify(obj.identities[0]);
  return JSON.stringify(obj);
}

async function supprimerCle(id) {
  if (!confirm("Supprimer definitivement cette cle ? Les messages qui lui sont destines deviendront illisibles.")) return;
  const r = await poster("/keys/delete", { id });
  message(r.success ? r.message : (r.error || "Suppression impossible."), !r.success);
  if (r.success) {
    for (const [k, v] of ouvertes) if (v._fichier === Number(id)) ouvertes.delete(k);
    await chargerCles();
  }
}

// ── Contacts (locaux au navigateur) ───────────────────────────────
function chargerContacts() {
  try { contacts = JSON.parse(localStorage.getItem(CLE_CONTACTS) || "[]"); }
  catch { contacts = []; }
  rendreContacts(); rendreSelecteurs();
}
function sauverContacts() {
  try { localStorage.setItem(CLE_CONTACTS, JSON.stringify(contacts)); }
  catch { message("Stockage local indisponible : les contacts ne seront pas conserves.", true); }
}
function rendreContacts() {
  const box = $("liste-contacts");
  if (!contacts.length) { box.innerHTML = `<div class="vide">Aucun contact.</div>`; return; }
  box.innerHTML = contacts.map((c, i) => `<div class="cle">
      <div><div class="cle-nom">${esc(c.nom)}</div><div class="cle-id">${esc(c.keyId)}</div></div>
      <div class="cle-actions"><button class="btn btn-d" data-suppr-contact="${i}">Retirer</button></div>
    </div>`).join("");
}
function ajouterContact() {
  const nom = $("co-nom").value.trim();
  const bloc = $("co-bloc").value.trim();
  if (!nom || !bloc) { message("Nom et cle publique requis.", true); return; }
  const pub = Q.parsePublicKeyBlock(bloc);
  if (!pub) { message("Bloc de cle publique invalide.", true); return; }
  if (contacts.some((c) => c.keyId === pub.keyId)) { message("Ce contact existe deja.", true); return; }
  contacts.push({
    nom, keyId: pub.keyId,
    kemPublicKey: Q.bytesToB64(pub.kemPublicKey),
    dsaPublicKey: Q.bytesToB64(pub.dsaPublicKey),
  });
  sauverContacts(); rendreContacts(); rendreSelecteurs();
  $("co-nom").value = ""; $("co-bloc").value = "";
  message("Contact « " + nom + " » ajoute.");
}
function contactVersPub(c) {
  return {
    keyId: c.keyId,
    kemPublicKey: Q.b64ToBytes(c.kemPublicKey),
    dsaPublicKey: Q.b64ToBytes(c.dsaPublicKey),
  };
}

// ── Selecteurs ────────────────────────────────────────────────────
function rendreSelecteurs() {
  const mesCles = [...ouvertes.values()];
  const opts = mesCles.map((i) => `<option value="${esc(i.keyId)}">${esc(i.keyId)}</option>`).join("");
  $("ch-signeur").innerHTML = `<option value="">Ne pas signer</option>` + opts;
  $("si-cle").innerHTML = opts || `<option value="">Deverrouillez une cle d'abord</option>`;

  const dest = contacts.map((c) => `<option value="${esc(c.keyId)}">${esc(c.nom)} — ${esc(c.keyId)}</option>`);
  // On peut aussi s'ecrire a soi-meme : pratique pour archiver une note.
  const soi = mesCles.map((i) => `<option value="${esc(i.keyId)}">Moi — ${esc(i.keyId)}</option>`);
  $("ch-dest").innerHTML = [...dest, ...soi].join("") ||
    `<option value="" disabled>Ajoutez un contact</option>`;
}

function pubDepuisKeyId(keyId) {
  const c = contacts.find((x) => x.keyId === keyId);
  if (c) return contactVersPub(c);
  const mien = ouvertes.get(keyId);
  return mien ? Q.toPublicIdentity(mien) : null;
}

// ── Chiffrer / dechiffrer ─────────────────────────────────────────
async function chiffrer() {
  const dest = [...$("ch-dest").selectedOptions].map((o) => o.value).filter(Boolean);
  if (!dest.length) { message("Choisissez au moins un destinataire.", true); return; }
  const texte = $("ch-texte").value;
  if (!texte) { message("Le message est vide.", true); return; }
  const signeurId = $("ch-signeur").value;
  const btn = $("btn-chiffrer");
  btn.disabled = true; btn.innerHTML = '<span class="spin"></span> Chiffrement…';
  try {
    const pubs = dest.map(pubDepuisKeyId).filter(Boolean);
    if (!pubs.length) { message("Destinataires introuvables.", true); return; }
    const opts = signeurId && ouvertes.has(signeurId) ? { signer: ouvertes.get(signeurId) } : {};
    $("ch-sortie").value = await Q.encryptMessage(pubs, texte, opts);
    message("Message chiffre pour " + pubs.length + " destinataire(s).");
  } catch (e) {
    message("Chiffrement impossible : " + e.message, true);
  } finally {
    btn.disabled = false; btn.textContent = "Chiffrer";
  }
}

async function dechiffrer() {
  const bloc = $("de-entree").value.trim();
  if (!bloc) { message("Collez un bloc QSeal.", true); return; }
  if (!ouvertes.size) { message("Deverrouillez d'abord une de vos cles.", true); return; }
  const etat = $("de-etat");
  etat.innerHTML = '<span class="spin"></span>';
  try {
    const r = await Q.decryptMessage(bloc, [...ouvertes.values()], pubDepuisKeyId);
    if (r.status !== "decrypted") {
      $("de-sortie").value = "";
      const libelle = {
        "no-key": "Ce message ne vous est pas destine (aucune de vos cles ne figure parmi les destinataires).",
        "corrupt": "Bloc illisible ou tronque.",
      }[r.status] || ("Echec : " + r.status);
      etat.innerHTML = `<span class="puce p-err">${esc(libelle)}</span>`;
      return;
    }
    $("de-sortie").value = r.plaintext;
    let sig;
    if (!r.signerKeyId) {
      sig = `<span class="puce p-neutre">non signe</span>`;
    } else if (r.signatureValid && r.signerKnown) {
      const nom = (contacts.find((c) => c.keyId === r.signerKeyId) || {}).nom || r.signerKeyId;
      sig = `<span class="puce p-ok">signature valide — ${esc(nom)}</span>`;
    } else if (r.signatureValid) {
      sig = `<span class="puce p-warn">signature valide, signataire inconnu (${esc(r.signerKeyId)})</span>`;
    } else {
      sig = `<span class="puce p-err">signature invalide</span>`;
    }
    etat.innerHTML = `<span class="puce p-ok">dechiffre</span> ${sig}`;
  } catch (e) {
    etat.innerHTML = `<span class="puce p-err">${esc("Echec : " + e.message)}</span>`;
  }
}

// ── Signer / verifier ─────────────────────────────────────────────
async function signer() {
  const keyId = $("si-cle").value;
  const id = ouvertes.get(keyId);
  if (!id) { message("Deverrouillez une cle pour signer.", true); return; }
  const texte = $("si-texte").value;
  if (!texte) { message("Le texte est vide.", true); return; }
  try {
    $("si-sortie").value = await Q.signPlainBlock(texte, id);
    message("Texte signe.");
  } catch (e) { message("Signature impossible : " + e.message, true); }
}

async function verifier() {
  const bloc = $("ve-entree").value.trim();
  if (!bloc) { message("Collez un bloc signe.", true); return; }
  const etat = $("ve-etat");
  try {
    const r = await Q.verifyPlainBlock(bloc, pubDepuisKeyId);
    if (!r || !r.plaintext) { etat.innerHTML = `<span class="puce p-err">Bloc invalide.</span>`; return; }
    const nom = (contacts.find((c) => c.keyId === r.signerKeyId) || {}).nom || r.signerKeyId;
    const puce = r.valid
      ? (r.signerKnown
          ? `<span class="puce p-ok">signature valide — ${esc(nom)}</span>`
          : `<span class="puce p-warn">signature valide, signataire inconnu (${esc(r.signerKeyId)})</span>`)
      : `<span class="puce p-err">signature invalide</span>`;
    etat.innerHTML = `${puce}<div class="champ" style="margin-top:10px">
        <label class="et">Texte signe</label><textarea readonly>${esc(r.plaintext)}</textarea></div>`;
  } catch (e) {
    etat.innerHTML = `<span class="puce p-err">${esc("Echec : " + e.message)}</span>`;
  }
}

// ── Export cle publique / sauvegarde ──────────────────────────────
function montrerBloc(titre, contenu) {
  const w = window.open("", "_blank", "width=640,height=520");
  if (!w) { navigator.clipboard.writeText(contenu); message(titre + " copie dans le presse-papiers."); return; }
  w.document.write(`<title>${esc(titre)}</title>
    <body style="font-family:monospace;padding:18px;white-space:pre-wrap;word-break:break-all">
    <h3 style="font-family:sans-serif">${esc(titre)}</h3>${esc(contenu)}</body>`);
}

async function copier(idChamp) {
  const v = $(idChamp).value;
  if (!v) { message("Rien a copier.", true); return; }
  try { await navigator.clipboard.writeText(v); message("Copie dans le presse-papiers."); }
  catch { message("Copie refusee par le navigateur.", true); }
}

// ── Branchements ──────────────────────────────────────────────────
document.querySelectorAll(".onglet").forEach((b) => b.addEventListener("click", () => {
  document.querySelectorAll(".onglet").forEach((x) => x.classList.toggle("on", x === b));
  document.querySelectorAll(".vue").forEach((v) => v.classList.remove("on"));
  $("vue-" + b.dataset.vue).classList.add("on");
}));

$("btn-gen").addEventListener("click", genererIdentite);
$("btn-recharger").addEventListener("click", chargerCles);
$("btn-importer").addEventListener("click", importerCle);
$("btn-chiffrer").addEventListener("click", chiffrer);
$("btn-dechiffrer").addEventListener("click", dechiffrer);
$("btn-signer").addEventListener("click", signer);
$("btn-verifier").addEventListener("click", verifier);
$("btn-ajouter-contact").addEventListener("click", ajouterContact);
$("btn-copier-ch").addEventListener("click", () => copier("ch-sortie"));
$("btn-copier-si").addEventListener("click", () => copier("si-sortie"));

document.addEventListener("click", async (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  if (b.dataset.ouvrir) await ouvrirCle(Number(b.dataset.ouvrir));
  else if (b.dataset.suppr) await supprimerCle(b.dataset.suppr);
  else if (b.dataset.supprContact !== undefined) {
    contacts.splice(Number(b.dataset.supprContact), 1);
    sauverContacts(); rendreContacts(); rendreSelecteurs();
  } else if (b.dataset.pub) {
    const id = [...ouvertes.values()].find((i) => i._fichier === Number(b.dataset.pub));
    if (id) montrerBloc("Cle publique QSeal", Q.exportPublicKeyBlock(Q.toPublicIdentity(id)));
  } else if (b.dataset.sauv) {
    const id = [...ouvertes.values()].find((i) => i._fichier === Number(b.dataset.sauv));
    if (!id) return;
    const pass = prompt("Phrase secrete pour proteger la sauvegarde :");
    if (!pass) { message("Sauvegarde annulee : une phrase secrete est requise.", true); return; }
    montrerBloc("Sauvegarde QSeal", await Q.encryptBackup(pass, identiteVersJson(id)));
  }
});

// ── Demarrage ─────────────────────────────────────────────────────
if (!Q) {
  message("Noyau cryptographique QSeal non charge.", true);
} else {
  chargerContacts();
  chargerCles();
}
})();
