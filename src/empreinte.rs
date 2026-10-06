// ══════════════════════════════════════════════════════════════════
// empreinte.rs — empreintes d'appareil (envoyees par static/fp.js)
//
// Chaque visite des pages login / dashboard / account / first_setup
// ajoute une ligne a log/empreintes.tsv. Les 20 composants sont compares
// un par un (poids ci-dessous) pour donner un POURCENTAGE de
// correspondance : un appareil connu reste reconnu apres une mise a jour
// du navigateur, un changement d'ecran, etc.
//
// Utilise par main.rs (reception + ligne de log) et admin.rs (/empreintes :
// sections Appareils, Logs et Revenus du panel).
// Conservation 90 jours : purge par ~/vex-securite/check_vex.sh.
// ══════════════════════════════════════════════════════════════════
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};

pub const FICHIER: &str = "log/empreintes.tsv";

/// Les 20 composants, dans l'ORDRE de static/fp.js, avec leur poids.
/// Materiel (carte graphique, canvas, emojis, audio, polices) = poids fort :
/// quasi impossible a changer sans changer de machine. Reglages faciles a
/// modifier (langue, plugins, stockage) = poids faible.
pub const COMPOSANTS: [(&str, u32); 20] = [
    ("Navigateur", 4),
    ("Système", 6),
    ("Langues", 3),
    ("Plateforme", 3),
    ("Cœurs CPU", 5),
    ("Mémoire", 3),
    ("Tactile", 4),
    ("Écran", 6),
    ("Couleurs / zoom", 3),
    ("Fuseau horaire", 3),
    ("Carte graphique", 9),
    ("Paramètres WebGL", 6),
    ("Polices", 8),
    ("Canvas", 9),
    ("Emojis", 8),
    ("Audio", 8),
    ("Maths", 4),
    ("Affichage", 3),
    ("Stockage", 2),
    ("Plugins", 3),
];

/// Composants qui dependent de la MACHINE et pas du navigateur (index dans
/// COMPOSANTS, poids) : le meme PC vu par Chrome, Edge et Firefox doit
/// rester UNE machine. Canvas, audio, maths, parametres WebGL, plugins…
/// changent d'un navigateur a l'autre : ils sont exclus. La memoire pese
/// peu (Firefox ne la donne pas).
const MATERIEL: [(usize, u32); 10] = [
    (1, 6),  // Systeme
    (2, 2),  // Langues
    (3, 3),  // Plateforme
    (4, 5),  // Coeurs CPU
    (5, 1),  // Memoire
    (6, 4),  // Tactile
    (7, 6),  // Ecran
    (9, 3),  // Fuseau horaire
    (10, 6), // Carte graphique
    (12, 8), // Polices installees
];

/// Seuil (sur le materiel seulement) pour regrouper deux appareils en une
/// meme machine.
pub const SEUIL_MATERIEL: u32 = 80;

/// Partage suspect : au moins 3 machines vues depuis au moins 3 reseaux
/// differents, ou 5 machines et plus (un PC, un telephone et une tablette
/// a la maison ne declenchent rien).
const SUSPECT_MACHINES: usize = 3;
const SUSPECT_RESEAUX: usize = 3;
const SUSPECT_MACHINES_SEULES: usize = 5;

/// Ressemblance ponderee sur les composants materiels uniquement.
pub fn similarite_machine(a: &[String], b: &[String]) -> u32 {
    if a.len() != COMPOSANTS.len() || b.len() != COMPOSANTS.len() {
        return 0;
    }
    let total: u32 = MATERIEL.iter().map(|(_, p)| p).sum();
    let communs: u32 = MATERIEL.iter().filter(|(i, _)| a[*i] == b[*i]).map(|(_, p)| p).sum();
    ((communs as f64 / total as f64) * 100.0).round() as u32
}

/// Navigateur pilote par un programme (tests, robots) : compte a part.
pub fn est_automatise(l: &Ligne) -> bool {
    let ua = l.ua.to_lowercase();
    ua.contains("headless") || ua.contains("puppeteer") || ua.contains("playwright") || ua.contains("selenium")
}

/// Reseau d'une IP : /24 en IPv4, /48 en IPv6 (une box change parfois
/// d'adresse dans le meme bloc).
fn reseau(ip: &str) -> String {
    if ip.contains(':') {
        ip.split(':').take(3).collect::<Vec<_>>().join(":")
    } else {
        ip.rsplitn(2, '.').nth(1).unwrap_or(ip).to_string()
    }
}

#[derive(Clone)]
pub struct Ligne {
    pub date: String,
    pub hash: String,
    pub ip: String,
    pub page: String,
    pub ua: String,
    pub detail: String,
    pub compte: String,
    pub comps: Vec<String>,
}

/// Lit log/empreintes.tsv. Colonnes : date, hash, ip, page, ua, detail,
/// compte, composants (20 x 8 hex separes par des virgules), puis les
/// pourcentages calcules a la reception.
pub fn lire() -> Vec<Ligne> {
    let contenu = std::fs::read_to_string(FICHIER).unwrap_or_default();
    contenu
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() < 6 {
                return None;
            }
            let col = |i: usize| c.get(i).map(|s| s.to_string()).unwrap_or_default();
            Some(Ligne {
                date: col(0),
                hash: col(1),
                ip: col(2),
                page: col(3),
                ua: col(4),
                detail: col(5),
                compte: col(6),
                comps: col(7)
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect(),
            })
        })
        .collect()
}

/// Pourcentage de correspondance pondere entre deux listes de composants.
/// 0 si l'une des deux n'a pas ses 20 composants (empreinte v1).
pub fn similarite(a: &[String], b: &[String]) -> u32 {
    if a.len() != COMPOSANTS.len() || b.len() != COMPOSANTS.len() {
        return 0;
    }
    let total: u32 = COMPOSANTS.iter().map(|(_, p)| p).sum();
    let communs: u32 = COMPOSANTS
        .iter()
        .enumerate()
        .filter(|(i, _)| a[*i] == b[*i])
        .map(|(_, (_, p))| p)
        .sum();
    ((communs as f64 / total as f64) * 100.0).round() as u32
}

/// Noms des composants qui different (pour expliquer un pourcentage).
pub fn differences(a: &[String], b: &[String]) -> Vec<&'static str> {
    if a.len() != COMPOSANTS.len() || b.len() != COMPOSANTS.len() {
        return vec![];
    }
    COMPOSANTS
        .iter()
        .enumerate()
        .filter(|(i, _)| a[*i] != b[*i])
        .map(|(_, (nom, _))| *nom)
        .collect()
}

/// Derniere version connue de chaque appareil (hash -> ligne la plus recente).
fn derniers_par_hash(lignes: &[Ligne]) -> HashMap<String, Ligne> {
    let mut m: HashMap<String, Ligne> = HashMap::new();
    for l in lignes {
        m.insert(l.hash.clone(), l.clone());
    }
    m
}

pub struct Correspondance {
    pub pct: u32,
    pub hash: String,
    pub compte: String,
}

/// Appareil connu (autre que `hash`) qui ressemble le plus a `comps`.
pub fn meilleure_correspondance(lignes: &[Ligne], hash: &str, comps: &[String]) -> Option<Correspondance> {
    derniers_par_hash(lignes)
        .into_values()
        .filter(|l| l.hash != hash)
        .map(|l| Correspondance { pct: similarite(comps, &l.comps), hash: l.hash, compte: l.compte })
        .max_by_key(|c| c.pct)
}

/// Correspondance avec les appareils DEJA utilises par ce compte :
/// 100 si l'appareil est deja connu pour ce compte, sinon le meilleur
/// pourcentage parmi ses appareils. None si le compte n'a aucun historique.
pub fn correspondance_compte(lignes: &[Ligne], compte: &str, hash: &str, comps: &[String]) -> Option<u32> {
    if compte.is_empty() {
        return None;
    }
    let siens: Vec<&Ligne> = lignes.iter().filter(|l| l.compte == compte).collect();
    if siens.is_empty() {
        return None;
    }
    if siens.iter().any(|l| l.hash == hash) {
        return Some(100);
    }
    siens.iter().map(|l| similarite(comps, &l.comps)).max()
}

/// Regroupe des appareils en "machines" d'apres le MATERIEL : meme PC avec
/// plusieurs navigateurs = une machine. Deux appareils sont aussi la meme
/// machine s'ils partagent une IP avec le meme systeme et le meme ecran.
fn machines(appareils: &[&Ligne]) -> Vec<Vec<Ligne>> {
    let mut groupes: Vec<Vec<Ligne>> = Vec::new();
    for a in appareils {
        let meme = |b: &Ligne| {
            similarite_machine(&a.comps, &b.comps) >= SEUIL_MATERIEL
                || (a.ip == b.ip && a.comps.len() == COMPOSANTS.len() && b.comps.len() == COMPOSANTS.len()
                    && a.comps[1] == b.comps[1] && a.comps[7] == b.comps[7])
        };
        match groupes.iter_mut().find(|g| g.iter().any(meme)) {
            Some(g) => g.push((*a).clone()),
            None => groupes.push(vec![(*a).clone()]),
        }
    }
    groupes
}

/// Petit resume d'une machine pour le panel : systeme, navigateurs, reseaux.
fn decrire(g: &[Ligne]) -> Value {
    let navigateurs: BTreeSet<String> = g
        .iter()
        .map(|l| l.detail.split(" · ").nth(1).unwrap_or("?").to_string())
        .collect();
    let systeme = g.first().map(|l| l.detail.split(" · ").next().unwrap_or("?").to_string()).unwrap_or_default();
    let reseaux: BTreeSet<String> = g.iter().map(|l| reseau(&l.ip)).collect();
    json!({
        "systeme": systeme,
        "navigateurs": navigateurs,
        "reseaux": reseaux,
        "derniere": g.iter().map(|l| l.date.clone()).max().unwrap_or_default(),
    })
}

/// Resume pour le panel admin : appareils, comptes et dernieres visites.
pub fn resume_admin() -> Value {
    let lignes = lire();
    let derniers = derniers_par_hash(&lignes);

    // ── Appareils ──────────────────────────────────────────────────
    let mut appareils: Vec<Value> = derniers
        .values()
        .map(|dern| {
            let siennes: Vec<&Ligne> = lignes.iter().filter(|l| l.hash == dern.hash).collect();
            let ips: BTreeSet<&str> = siennes.iter().map(|l| l.ip.as_str()).collect();
            let comptes: BTreeSet<&str> = siennes
                .iter()
                .map(|l| l.compte.as_str())
                .filter(|c| !c.is_empty())
                .collect();
            let pages: BTreeSet<&str> = siennes.iter().map(|l| l.page.as_str()).collect();
            let proche = meilleure_correspondance(&lignes, &dern.hash, &dern.comps);
            let diff = proche
                .as_ref()
                .and_then(|p| derniers.get(&p.hash))
                .map(|p| differences(&dern.comps, &p.comps))
                .unwrap_or_default();
            json!({
                "hash": dern.hash,
                "premiere": siennes.first().map(|l| l.date.clone()).unwrap_or_default(),
                "derniere": dern.date,
                "visites": siennes.len(),
                "ips": ips,
                "comptes": comptes,
                "pages": pages,
                "ua": dern.ua,
                "detail": dern.detail,
                "proche_pct": proche.as_ref().map(|p| p.pct),
                "proche_hash": proche.as_ref().map(|p| p.hash.clone()),
                "proche_compte": proche.as_ref().map(|p| p.compte.clone()),
                "proche_diff": diff,
            })
        })
        .collect();
    appareils.sort_by(|a, b| b["derniere"].as_str().cmp(&a["derniere"].as_str()));

    // ── Comptes : combien de machines differentes par compte ───────
    let mut par_compte: HashMap<String, Vec<&Ligne>> = HashMap::new();
    for dern in derniers.values() {
        let comptes: BTreeSet<&str> = lignes
            .iter()
            .filter(|x| x.hash == dern.hash && !x.compte.is_empty())
            .map(|x| x.compte.as_str())
            .collect();
        for c in comptes {
            par_compte.entry(c.to_string()).or_default().push(dern);
        }
    }
    let mut comptes: Vec<Value> = par_compte
        .iter()
        .map(|(compte, apps)| {
            // Les navigateurs automatises (tests, robots) sont comptes a part.
            let (auto, humains): (Vec<&Ligne>, Vec<&Ligne>) = apps.iter().partition(|l| est_automatise(l));
            let groupes = machines(&humains);
            // Ressemblance moyenne du MATERIEL entre les machines du compte :
            // proche de 100 % = memes machines ; tres bas = sans rapport.
            let mut paires = Vec::new();
            for i in 0..groupes.len() {
                for j in (i + 1)..groupes.len() {
                    paires.push(similarite_machine(&groupes[i][0].comps, &groupes[j][0].comps));
                }
            }
            let moyenne = if paires.is_empty() { 100 } else { paires.iter().sum::<u32>() / paires.len() as u32 };
            let reseaux: BTreeSet<String> = humains.iter().map(|l| reseau(&l.ip)).collect();
            let n = groupes.len();
            let suspect = (n >= SUSPECT_MACHINES && reseaux.len() >= SUSPECT_RESEAUX) || n >= SUSPECT_MACHINES_SEULES;
            let raison = if !suspect {
                String::new()
            } else if n >= SUSPECT_MACHINES_SEULES {
                format!("{} machines différentes", n)
            } else {
                format!("{} machines depuis {} réseaux différents", n, reseaux.len())
            };
            json!({
                "compte": compte,
                "empreintes": apps.len(),
                "machines": n,
                "reseaux": reseaux.len(),
                "automatises": auto.len(),
                "ressemblance": moyenne,
                "partage_suspect": suspect,
                "raison": raison,
                "machines_detail": groupes.iter().map(|g| decrire(g)).collect::<Vec<_>>(),
            })
        })
        .collect();
    comptes.sort_by(|a, b| b["machines"].as_u64().cmp(&a["machines"].as_u64()));

    // ── Dernieres visites (section Logs) ───────────────────────────
    let contenu = std::fs::read_to_string(FICHIER).unwrap_or_default();
    let recents: Vec<Value> = contenu
        .lines()
        .rev()
        .take(100)
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() < 6 {
                return None;
            }
            let col = |i: usize| c.get(i).map(|s| s.to_string()).unwrap_or_default();
            Some(json!({
                "date": col(0), "hash": col(1), "ip": col(2), "page": col(3),
                "detail": col(5), "compte": col(6),
                "pct_compte": col(8).parse::<u32>().ok(),
                "pct_proche": col(9).parse::<u32>().ok(),
                "proche_hash": col(10),
            }))
        })
        .collect();

    json!({
        "appareils": appareils,
        "comptes": comptes,
        "recents": recents,
        "composants": COMPOSANTS.iter().map(|(n, p)| json!({"nom": n, "poids": p})).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(ip: &str, ua: &str, comps: &[&str]) -> Ligne {
        Ligne {
            date: "2026-10-06".into(), hash: format!("{:?}", comps), ip: ip.into(), page: "login".into(), ua: ua.into(),
            detail: "Windows · Chrome".into(), compte: "1".into(), comps: comps.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Composants : on change seulement ceux qui dependent du navigateur.
    fn pc(nav: &str) -> Vec<&'static str> {
        let nav: &'static str = Box::leak(nav.to_string().into_boxed_str());
        vec![nav, "win", "fr", "x64", "8", "8", "0", "1920", "24", "paris", "rtx", nav, "polices", nav, nav, nav, nav, nav, nav, nav]
    }

    #[test]
    fn meme_pc_plusieurs_navigateurs_une_machine() {
        let a = ligne("1.2.3.4", "Chrome", &pc("chrome"));
        let b = ligne("1.2.3.4", "Edge", &pc("edge"));
        let c = ligne("1.2.3.9", "Firefox", &pc("firefox"));
        assert!(similarite(&a.comps, &b.comps) < 70, "l'ancien calcul les separait");
        assert_eq!(machines(&[&a, &b, &c]).len(), 1, "un seul PC");
    }

    #[test]
    fn navigateur_automatise_ignore() {
        assert!(est_automatise(&ligne("1.1.1.1", "Mozilla/5.0 HeadlessChrome/141", &pc("x"))));
        assert!(!est_automatise(&ligne("1.1.1.1", "Mozilla/5.0 Chrome/141", &pc("x"))));
    }
}
