// ══════════════════════════════════════════════════════════════════
// vex-crypto.js — Module cryptographique client VEX
// v2.1 — IV intégré au blob chiffré — aucun champ IV séparé en DB
//
// PRINCIPES :
//   • Le mot de passe en clair NE QUITTE JAMAIS le navigateur
//   • Login/inscription → PBKDF2-SHA256(mdp, salt=email, 100k iter) → hex 64 car.
//   • Chiffrement fichiers → AES-256-GCM
//       Clé = HKDF(mdp, salt="VEX-file-salt-static", info="VEX-ExoDrive-file-v1")
//       IV  = aléatoire, préfixé au contenu chiffré
//       → IV jamais stocké en DB, transporté dans le blob chiffré
//   • Chiffrement requêtes sensibles → AES-256-GCM (clé dérivée du cookie session
//     XOR mdp en clair) — pour chiffrer les corps de requête contenant des données
//     sensibles (ex: contenu de fichiers, notes, messages)
//   • IDs, dates, emails, noms d'utilisateur → NON chiffrés (métadonnées structurelles)
//   • Seules les données "contenu" sont chiffrées (fichiers, champs marqués sensibles)
//
// USAGE :
//   <script src="/static/js/vex-crypto.js"></script>
//   Expose window.VEX — API décrite en bas de fichier.
//
// DÉPENDANCES : aucune (WebCrypto natif, disponible dans tous les navigateurs modernes)
// ══════════════════════════════════════════════════════════════════

(function (global) {
    'use strict';

    // ── Constantes ────────────────────────────────────────────────
    const PBKDF2_ITERATIONS = 100_000;
    const PBKDF2_HASH       = 'SHA-256';
    const PBKDF2_KEY_BITS   = 256;
    const HKDF_HASH         = 'SHA-256';
    const AES_MODE          = 'AES-GCM';
    const AES_KEY_BITS      = 256;
    const IV_BYTES          = 12;   // 96 bits — recommandé AES-GCM
    const TAG_BYTES         = 16;   // 128 bits — défaut AES-GCM
    const FILE_MAGIC        = new Uint8Array([0x56, 0x45, 0x58, 0x31]); // "VEX1"

    // Infos de contexte HKDF — distingue chaque usage de clé
    const HKDF_INFO_FILE    = 'VEX-ExoDrive-file-v1';
    const HKDF_INFO_REQUEST = 'VEX-request-body-v1';
    const HKDF_SALT_FILE    = 'VEX-file-salt-static';

    // ── État interne (jamais exposé directement) ──────────────────
    let _mdpClair    = null;   // mot de passe en clair — gardé en mémoire JS uniquement
    let _emailClair  = null;   // email en clair — nécessaire pour dériver l'IV
    let _cookieCache = null;   // valeur du cookie session (lue une fois, mise en cache)

    // ── Utilitaires bas niveau ────────────────────────────────────

    const enc = new TextEncoder();
    const dec = new TextDecoder();

    function strToBytes(str) {
        return enc.encode(str);
    }

    function bytesToHex(buf) {
        return Array.from(new Uint8Array(buf))
            .map(b => b.toString(16).padStart(2, '0'))
            .join('');
    }

    function hexToBytes(hex) {
        if (hex.length % 2 !== 0) throw new Error('Hex string invalide');
        const arr = new Uint8Array(hex.length / 2);
        for (let i = 0; i < hex.length; i += 2) {
            arr[i / 2] = parseInt(hex.slice(i, i + 2), 16);
        }
        return arr;
    }

    function bytesToBase64(bytes) {
        let bin = '';
        for (const b of bytes) bin += String.fromCharCode(b);
        return btoa(bin);
    }

    function base64ToBytes(b64) {
        const bin = atob(b64);
        const arr = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
        return arr;
    }

    // XOR octet-à-octet de deux Uint8Array de même longueur
    function xorBytes(a, b) {
        if (a.length !== b.length) throw new Error('XOR : longueurs différentes');
        const out = new Uint8Array(a.length);
        for (let i = 0; i < a.length; i++) out[i] = a[i] ^ b[i];
        return out;
    }

    // Lit le cookie session depuis document.cookie (mis en cache)
    function getCookieSession() {
        if (_cookieCache !== null) return _cookieCache;
        const match = document.cookie.match(/(?:^|;\s*)connexion_cookie=([^;]+)/);
        _cookieCache = match ? match[1] : '';
        return _cookieCache;
    }

    // Invalide le cache cookie (utile après login/logout)
    function invalidateCookieCache() {
        _cookieCache = null;
    }

    // ── PBKDF2-SHA256 ─────────────────────────────────────────────
    async function pbkdf2Hex(password, salt) {
        const keyMaterial = await crypto.subtle.importKey(
            'raw', strToBytes(password), 'PBKDF2', false, ['deriveBits']
        );
        const bits = await crypto.subtle.deriveBits(
            {
                name:       'PBKDF2',
                hash:       PBKDF2_HASH,
                salt:       strToBytes(salt.toLowerCase()),
                iterations: PBKDF2_ITERATIONS,
            },
            keyMaterial,
            PBKDF2_KEY_BITS
        );
        return bytesToHex(bits);
    }

    // ── Dérivation de clé AES-256-GCM via HKDF ───────────────────
    async function hkdfDeriveAesKey(masterBytes, info, saltStr) {
        const keyMaterial = await crypto.subtle.importKey(
            'raw', masterBytes, 'HKDF', false, ['deriveKey']
        );
        return crypto.subtle.deriveKey(
            {
                name: 'HKDF',
                hash: HKDF_HASH,
                salt: strToBytes(saltStr),
                info: strToBytes(info),
            },
            keyMaterial,
            { name: AES_MODE, length: AES_KEY_BITS },
            false,
            ['encrypt', 'decrypt']
        );
    }

    // ── Dérivation de bits bruts via HKDF ─────────────────────────
    // Utilisé pour dériver l'IV (on veut des bytes, pas une CryptoKey)
    async function hkdfDeriveBytes(masterBytes, info, saltStr, lengthBits) {
        const keyMaterial = await crypto.subtle.importKey(
            'raw', masterBytes, 'HKDF', false, ['deriveBits']
        );
        const bits = await crypto.subtle.deriveBits(
            {
                name: 'HKDF',
                hash: HKDF_HASH,
                salt: strToBytes(saltStr),
                info: strToBytes(info),
            },
            keyMaterial,
            lengthBits
        );
        return new Uint8Array(bits);
    }

    // ── Clé de chiffrement fichiers ───────────────────────────────
    // Dérivée du mdp en clair uniquement (stable tant que le mdp ne change pas).
    async function getClesFichier() {
        if (!_mdpClair) throw new Error('VEX : non connecté (mdp en clair absent)');
        const mdpBytes = strToBytes(_mdpClair);
        return hkdfDeriveAesKey(mdpBytes, HKDF_INFO_FILE, HKDF_SALT_FILE);
    }

    // ── Clé de chiffrement requêtes ───────────────────────────────
    let _sessionHashBytes = null;

    async function getCleRequete() {
        if (!_mdpClair) throw new Error('VEX : non connecté');
        if (!_sessionHashBytes) throw new Error('VEX : setSessionHash() non appelé après login');

        const cookieStr = getCookieSession();
        if (!cookieStr) throw new Error('VEX : cookie session absent');

        const cookieHash = new Uint8Array(await crypto.subtle.digest(
            'SHA-256',
            strToBytes(cookieStr)
        ));

        const masterBytes = xorBytes(_sessionHashBytes, cookieHash);
        return hkdfDeriveAesKey(masterBytes, HKDF_INFO_REQUEST, HKDF_SALT_FILE);
    }

    // ── Chiffrement / Déchiffrement AES-256-GCM générique ─────────

    async function aesEncrypt(key, plaintext, iv) {
        // Si iv fourni → déterministe ; sinon → aléatoire (pour les requêtes)
        const ivUsed = iv || crypto.getRandomValues(new Uint8Array(IV_BYTES));
        const encrypted = await crypto.subtle.encrypt({ name: AES_MODE, iv: ivUsed }, key, plaintext);
        return { iv: ivUsed, data: new Uint8Array(encrypted) };
    }

    async function aesDecrypt(key, iv, data) {
        const plain = await crypto.subtle.decrypt({ name: AES_MODE, iv }, key, data);
        return new Uint8Array(plain);
    }

    // ── Sérialisation du payload chiffré ──────────────────────────
    // Format : base64(iv_12bytes || ciphertext)
    function packEncrypted(iv, data) {
        const packed = new Uint8Array(IV_BYTES + data.length);
        packed.set(iv, 0);
        packed.set(data, IV_BYTES);
        return bytesToBase64(packed);
    }

    function unpackEncrypted(b64) {
        const packed = base64ToBytes(b64);
        if (packed.length < IV_BYTES + TAG_BYTES) throw new Error('Payload chiffré trop court');
        return {
            iv:   packed.slice(0, IV_BYTES),
            data: packed.slice(IV_BYTES),
        };
    }

    // ══════════════════════════════════════════════════════════════
    // API publique — window.VEX
    // ══════════════════════════════════════════════════════════════
    const VEX = {

        // ── Gestion de session ────────────────────────────────────

        /**
         * À appeler après un login réussi.
         * @param {string} mdpClair     Mot de passe en clair.
         * @param {string} pbkdf2HexStr Hash PBKDF2 hex 64 car. déjà calculé au login.
         * @param {string} email        Email de l'utilisateur (pour dériver les IV fichiers).
         */
        setSession(mdpClair, pbkdf2HexStr, email) {
            _mdpClair         = mdpClair;
            _emailClair       = email.toLowerCase();
            _sessionHashBytes = hexToBytes(pbkdf2HexStr);
            invalidateCookieCache();
        },

        /**
         * À appeler au logout — efface le mdp et les clés de la mémoire.
         */
        clearSession() {
            _mdpClair         = null;
            _emailClair       = null;
            _sessionHashBytes = null;
            _cookieCache      = null;
        },

        /**
         * Indique si une session est active.
         * @returns {boolean}
         */
        isConnected() {
            return _mdpClair !== null;
        },

        // ── Hash serveur (login / inscription) ────────────────────

        /**
         * Hash PBKDF2-SHA256 à envoyer au serveur.
         * Le mot de passe en clair ne quitte jamais le navigateur.
         * @param {string} password Mot de passe en clair.
         * @param {string} email    Email de l'utilisateur (sert de salt).
         * @returns {Promise<string>} Hash hex 64 caractères.
         */
        hashPourServeur: pbkdf2Hex,

        // ── Chiffrement / Déchiffrement fichiers (ExoDrive) ───────

        /**
         * Chiffre un File avant upload.
         * Le blob retourné contient IV || ciphertext, donc aucun champ IV séparé
         * n'est nécessaire en DB.
         *
         * @param {File} file Fichier à chiffrer.
         * @returns {Promise<{blob: Blob}>}  Blob chiffré prêt à uploader.
         */
        async chiffrerFichier(file) {
            if (!_emailClair) throw new Error('VEX : email absent — appelez setSession(mdp, hash, email)');
            const key   = await getClesFichier();
            const plain = new Uint8Array(await file.arrayBuffer());
            const { iv, data } = await aesEncrypt(key, plain);
            const packed = new Uint8Array(FILE_MAGIC.length + IV_BYTES + data.length);
            packed.set(FILE_MAGIC, 0);
            packed.set(iv, FILE_MAGIC.length);
            packed.set(data, FILE_MAGIC.length + IV_BYTES);
            return {
                blob: new Blob([packed], { type: 'application/octet-stream' }),
            };
        },

        /**
         * Déchiffre un blob reçu du serveur.
         * Le blob doit contenir IV || ciphertext.
         *
         * @param {Blob} blob Blob chiffré reçu du serveur.
         * @param {string} mimeType Type MIME original du fichier.
         * @returns {Promise<Blob>} Blob du fichier original.
         */
        async dechiffrerFichier(blob, mimeType) {
            if (!_emailClair) throw new Error('VEX : email absent — appelez setSession(mdp, hash, email)');
            const key   = await getClesFichier();
            const packed = new Uint8Array(await blob.arrayBuffer());
            const minLen = FILE_MAGIC.length + IV_BYTES + TAG_BYTES;
            if (packed.length < minLen) throw new Error('Fichier non chiffré ou trop court');
            for (let i = 0; i < FILE_MAGIC.length; i++) {
                if (packed[i] !== FILE_MAGIC[i]) throw new Error('Fichier non chiffré VEX');
            }
            const iv = packed.slice(FILE_MAGIC.length, FILE_MAGIC.length + IV_BYTES);
            const data = packed.slice(FILE_MAGIC.length + IV_BYTES);
            const plain = await aesDecrypt(key, iv, data);
            return new Blob([plain], { type: mimeType || 'application/octet-stream' });
        },

        // ── Chiffrement des corps de requêtes sensibles ───────────

        /**
         * Chiffre les valeurs sensibles d'un objet avant envoi au serveur.
         * @param {object}   payload        Objet à envoyer.
         * @param {string[]} sensitiveKeys  Liste des clés dont la valeur est chiffrée.
         * @returns {Promise<object>}
         */
        async chiffrerPayload(payload, sensitiveKeys) {
            const key    = await getCleRequete();
            const result = { ...payload };
            for (const k of sensitiveKeys) {
                if (result[k] === undefined || result[k] === null) continue;
                const raw = typeof result[k] === 'string'
                    ? strToBytes(result[k])
                    : strToBytes(JSON.stringify(result[k]));
                const { iv, data } = await aesEncrypt(key, raw); // IV aléatoire pour les requêtes
                result[k] = packEncrypted(iv, data);
            }
            return result;
        },

        /**
         * Déchiffre les valeurs d'un objet reçu du serveur.
         * @param {object}   payload        Objet reçu.
         * @param {string[]} sensitiveKeys  Clés à déchiffrer.
         * @param {boolean}  [parseJson]    Si true, parse les valeurs déchiffrées en JSON.
         * @returns {Promise<object>}
         */
        async dechiffrerPayload(payload, sensitiveKeys, parseJson = false) {
            const key    = await getCleRequete();
            const result = { ...payload };
            for (const k of sensitiveKeys) {
                if (!result[k]) continue;
                const { iv, data } = unpackEncrypted(result[k]);
                const plain = await aesDecrypt(key, iv, data);
                const str   = dec.decode(plain);
                result[k]   = parseJson ? JSON.parse(str) : str;
            }
            return result;
        },

        /**
         * Chiffre un Uint8Array brut (usage générique).
         * @param {Uint8Array} plaintext
         * @returns {Promise<{iv: Uint8Array, data: Uint8Array}>}
         */
        async chiffrer(plaintext) {
            const key = await getClesFichier();
            return aesEncrypt(key, plaintext);
        },

        /**
         * Déchiffre des bytes chiffrés (usage générique).
         * @param {Uint8Array} iv
         * @param {Uint8Array} data
         * @returns {Promise<Uint8Array>}
         */
        async dechiffrer(iv, data) {
            const key = await getClesFichier();
            return aesDecrypt(key, iv, data);
        },

        // ── Requête fetch avec chiffrement automatique ────────────

        /**
         * Équivalent de fetch() avec chiffrement transparent du corps.
         */
        async fetch(url, options = {}) {
            const {
                body        = null,
                encryptKeys = [],
                decryptKeys = [],
                decryptAsJson = false,
                ...fetchOptions
            } = options;

            let bodyToSend = body;
            if (body && encryptKeys.length > 0) {
                bodyToSend = await this.chiffrerPayload(body, encryptKeys);
            }

            const response = await fetch(url, {
                ...fetchOptions,
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(fetchOptions.headers || {}),
                },
                body: bodyToSend ? JSON.stringify(bodyToSend) : undefined,
            });

            if (!response.ok) {
                const err = await response.json().catch(() => ({}));
                throw Object.assign(new Error(err.error || `HTTP ${response.status}`), { status: response.status, data: err });
            }

            const data = await response.json();

            if (decryptKeys.length > 0) {
                return this.dechiffrerPayload(data, decryptKeys, decryptAsJson);
            }
            return data;
        },

        // ── Utilitaires exposés ───────────────────────────────────

        /** Convertit un Uint8Array en hex string */
        toHex: bytesToHex,

        /** Convertit un hex string en Uint8Array */
        fromHex: hexToBytes,

        /** Convertit un Uint8Array en base64 */
        toBase64: bytesToBase64,

        /** Convertit un base64 en Uint8Array */
        fromBase64: base64ToBytes,
    };

    global.VEX = VEX;

})(window);

// ══════════════════════════════════════════════════════════════════
// GUIDE D'INTÉGRATION RAPIDE v2.0
// ══════════════════════════════════════════════════════════════════
//
// 1. CONNEXION (login.html) :
//
//    const hash = await VEX.hashPourServeur(mdpClair, email);
//    const r = await fetch('/login/login', {
//        method: 'POST',
//        body: new URLSearchParams({ action:'login', email, motdepass: hash }),
//        credentials: 'include',
//    });
//    const d = await r.json();
//    if (d.success) {
//        VEX.setSession(mdpClair, hash, email);  // ← email requis en v2.0
//        window.location.href = d.redirect;
//    }
//
// 2. UPLOAD FICHIER (fchier.html) :
//
//    const { blob } = await VEX.chiffrerFichier(file);
//    const bytes = new Uint8Array(await blob.arrayBuffer());
//    const file_b64 = VEX.toBase64(bytes);
//    await fetch('/api/fchier/upload', {
//        method: 'POST',
//        credentials: 'include',
//        headers: { 'Content-Type': 'application/json' },
//        body: JSON.stringify({ file_name, file_b64, mime_type, taille: bytes.length, visble, current_folder }),
//    });
//
//    Le blob envoyé contient déjà IV || ciphertext.
//
// 3. DOWNLOAD FICHIER :
//
//    const r = await fetch(`/api/fchier/download?id=${item.id}`, { credentials:'include' });
//    const data = await r.json();  // { success, contenu (base64), mime, nom }
//    const blobChiffre = new Blob([VEX.fromBase64(data.contenu)]);
//    const blobClair   = await VEX.dechiffrerFichier(blobChiffre, data.mime);
//
// 4. REQUÊTE AVEC CHAMPS SENSIBLES :
//
//    const r = await VEX.fetch('/api/notes/save', {
//        method: 'POST',
//        body: { note_id: 5, title: 'Mon titre', content: 'Texte secret' },
//        encryptKeys:  ['content'],
//        decryptKeys:  ['saved_at'],
//    });
//
// 5. LOGOUT :
//
//    VEX.clearSession();
//    await fetch('/api/login/logout', { method:'POST', credentials:'include' });
//    window.location.href = '/login';
//
// ══════════════════════════════════════════════════════════════════
// CE QUI EST CHIFFRÉ vs CE QUI NE L'EST PAS
//
//  Chiffré (AES-256-GCM) :
//    • Contenu des fichiers uploadés (ExoDrive)
//    • Valeurs des champs marqués sensitiveKeys dans VEX.fetch()
//
//  Non chiffré (transit en clair sur HTTPS) :
//    • IDs (file_id, user_id, note_id…)
//    • Dates / timestamps
//    • Emails, noms d'utilisateur
//    • Hash PBKDF2 (c'est son rôle — le serveur doit le comparer)
//    • Métadonnées de fichier (nom, taille, type MIME)
//    • L'IV N'EST PAS stocké à part — il est préfixé au blob chiffré
//
//  Aucun changement de schéma DB requis.
// ══════════════════════════════════════════════════════════════════
