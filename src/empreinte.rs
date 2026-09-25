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

/// Seuil au-dessus duquel deux empreintes sont la meme machine
/// (ex : meme PC apres une mise a jour de Firefox).
pub const SEUIL_MEME_MACHINE: u32 = 70;

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

/// Regroupe des empreintes en "machines" : deux empreintes a
/// SEUIL_MEME_MACHINE % ou plus sont la meme machine.
fn nb_machines(appareils: &[&Ligne]) -> usize {
    let mut groupes: Vec<Vec<&Ligne>> = Vec::new();
    for a in appareils {
        match groupes
            .iter_mut()
            .find(|g| g.iter().any(|b| similarite(&a.comps, &b.comps) >= SEUIL_MEME_MACHINE))
        {
            Some(g) => g.push(a),
            None => groupes.push(vec![a]),
        }
    }
    groupes.len()
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
            let machines = nb_machines(apps);
            // Ressemblance moyenne entre les appareils du compte : proche de
            // 100 % = memes machines ; tres bas = machines sans rapport.
            let mut paires = Vec::new();
            for i in 0..apps.len() {
                for j in (i + 1)..apps.len() {
                    paires.push(similarite(&apps[i].comps, &apps[j].comps));
                }
            }
            let moyenne = if paires.is_empty() {
                100
            } else {
                paires.iter().sum::<u32>() / paires.len() as u32
            };
            json!({
                "compte": compte,
                "empreintes": apps.len(),
                "machines": machines,
                "ressemblance": moyenne,
                // PC + telephone = 2 machines, normal. 3 ou plus = compte
                // possiblement partage entre plusieurs personnes.
                "partage_suspect": machines >= 3,
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
