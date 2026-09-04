// ══════════════════════════════════════════════════════════════════
// sync.rs — moteur de synchronisation recursive + conflits (v2).
//
// Synchronise recursivement toute l'arborescence de fchier (pas
// seulement la racine), et detecte les vrais conflits (fichier modifie
// des deux cotes depuis la derniere synchro connue) grace a un etat
// persistant (vex-sync-state.json) -- voir EtatSync. En cas de conflit,
// la version distante est enregistree a cote sous forme de copie
// "(conflit-serveur-...)", la version locale n'est JAMAIS ecrasee.
//
// Suppression VRAIMENT bidirectionnelle : un fichier connu (deja
// synchronise au moins une fois) qui disparait d'un cote est supprime de
// l'autre cote au prochain passage. Voir le commentaire de
// synchroniser_dossier() pour le detail complet des regles de decision.
//
// Isole dans son propre module (plutot que directement dans main.rs)
// pour pouvoir etre exerce par un test d'integration reel (voir
// src/bin/synctest.rs) sans dupliquer la logique.
// ══════════════════════════════════════════════════════════════════

use crate::api::{FileEntry, VexClient};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const STATE_FILE: &str = "vex-sync-state.json";

/// Etat connu d'un fichier a la fin de sa derniere synchronisation reussie
/// -- permet de distinguer "modifie depuis" de "jamais vu" des deux cotes,
/// donc de detecter un vrai conflit (modifie des DEUX cotes) plutot que de
/// pousser aveuglement une version sur l'autre a chaque passage.
#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
struct EtatFichier {
    remote_id: i64,
    /// Chaine "date" telle que renvoyee par le serveur (opaque, comparee
    /// uniquement pour egalite -- pas besoin de la parser).
    remote_date: String,
    local_taille: u64,
    local_mtime: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct EtatSync {
    /// Cle = chemin relatif depuis la racine du dossier synchronise, avec
    /// '/' comme separateur meme sous Windows (portable, sert de cle stable).
    fichiers: HashMap<String, EtatFichier>,
}

/// Un seul fichier d'etat sur disque, mais namespace par mapping (ex.
/// "Documents", "Images", ...) -- indispensable des qu'on synchronise
/// plusieurs dossiers locaux distincts : sans ca, un "notes.txt" a la
/// racine de "Documents" et un "notes.txt" a la racine de "Images"
/// partageraient la meme cle d'etat et se marcheraient dessus.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct EtatGlobal {
    mappings: HashMap<String, EtatSync>,
}

fn etat_path() -> PathBuf {
    directories::ProjectDirs::from("com", "vex", "vex-sync-client")
        .map(|d| d.config_dir().join(STATE_FILE))
        .unwrap_or_else(|| PathBuf::from(STATE_FILE))
}

fn charger_etat_global() -> EtatGlobal {
    std::fs::read_to_string(etat_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn sauver_etat_global(etat: &EtatGlobal) {
    let p = etat_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(etat) {
        let _ = std::fs::write(p, s);
    }
}

/// Reinitialise TOUT l'etat de synchronisation stocke sur disque (tous les
/// mappings). Utile pour un test qui simule plusieurs "PC" distincts avec
/// le meme dossier de config.
pub fn reinitialiser_etat() {
    let _ = std::fs::remove_file(etat_path());
}

pub fn deviner_mime(nom: &str) -> &'static str {
    match nom.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "txt" | "md" | "log" => "text/plain",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

fn mtime_secs(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn horodatage_fichier() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

/// Nom pour la copie conservee en cas de conflit : "rapport.docx" devient
/// "rapport (conflit-serveur-<horodatage>).docx".
fn nom_copie_conflit(nom: &str) -> String {
    match nom.rsplit_once('.') {
        Some((stem, ext)) if !ext.is_empty() && !stem.is_empty() => {
            format!("{stem} (conflit-serveur-{}).{ext}", horodatage_fichier())
        }
        _ => format!("{nom} (conflit-serveur-{})", horodatage_fichier()),
    }
}

/// Point d'entree pour UN mapping nomme (ex. "Documents" <-> dossier
/// distant "Documents" a la racine de fchier). `cle_espace` namespace
/// l'etat persistant pour ne jamais se melanger avec un autre mapping --
/// DOIT inclure le compte (ex. "email::Documents"), pas seulement le nom
/// du dossier : sinon, reconfigurer l'appli avec un AUTRE compte VEX tout
/// en reutilisant le meme nom de dossier ferait lire une baseline perimee
/// d'un compte different, et pourrait a tort supprimer des fichiers
/// locaux (vus comme "supprimes a distance" alors qu'ils n'ont juste
/// jamais existe sur ce nouveau compte).
pub fn synchroniser_mapping(client: &VexClient, cle_espace: &str, nom_affiche: &str, dossier_local: &Path, dossier_distant_id: i64, log: &dyn Fn(&str)) {
    log(&format!("Synchronisation de « {nom_affiche} » en cours..."));
    let mut global = charger_etat_global();
    let mut etat = global.mappings.remove(cle_espace).unwrap_or_default();
    synchroniser_dossier(client, dossier_local, dossier_distant_id, "", &mut etat, log);
    global.mappings.insert(cle_espace.to_string(), etat);
    sauver_etat_global(&global);
    log(&format!("Synchronisation de « {nom_affiche} » terminee."));
}

/// Point d'entree pour un usage a un seul dossier, synchronise a la
/// racine de fchier (dossier distant 0) -- conserve pour les tests et
/// l'usage le plus simple.
pub fn synchroniser(client: &VexClient, dossier_local: &Path, log: &dyn Fn(&str)) {
    synchroniser_mapping(client, "_racine", "_racine", dossier_local, 0, log);
}

/// Synchronise un niveau de l'arborescence, puis recurse dans les
/// sous-dossiers communs ou nouvellement crees.
///
/// Regles de decision par fichier (cle = chemin relatif depuis la racine
/// synchronisee, ex. "projets/rapport.docx") :
///   - present seulement en local, JAMAIS synchronise (pas d'etat) -> upload
///     (nouveau fichier local).
///   - present seulement a distance, JAMAIS synchronise (pas d'etat) ->
///     download (nouveau fichier distant).
///   - present seulement en local, MAIS deja synchronise avant (etat
///     present) -> le fichier distant a ete supprime depuis (par
///     l'utilisateur ou un autre appareil) : la suppression est PROPAGEE,
///     le fichier local est supprime a son tour.
///   - present seulement a distance, MAIS deja synchronise avant -> le
///     fichier local a ete supprime : la suppression est PROPAGEE, le
///     fichier distant est supprime a son tour.
///   - absent des deux cotes mais connu de l'etat -> supprime des deux
///     cotes (par exemple lors d'un appel precedent) : on arrete juste de
///     le suivre.
///   - present des les deux cotes, JAMAIS synchronise avant (etat absent) ->
///     on ne sait pas lequel est "le bon" : traite comme un conflit (voir
///     plus bas), rien n'est ecrase.
///   - present des les deux cotes AVEC un etat connu :
///       - local change depuis l'etat (taille/mtime differents) ET distant
///         change (date differente) -> CONFLIT : la version distante est
///         telechargee a cote sous forme de copie "(conflit-serveur-...)",
///         le fichier local n'est JAMAIS touche.
///       - local seul a change -> upload (ecrase le contenu distant).
///       - distant seul a change -> download (ecrase le fichier local).
///       - aucun des deux n'a change -> rien a faire.
///
/// Attention : une suppression est maintenant reellement propagee d'un
/// cote a l'autre (contrairement a la toute premiere version qui se
/// contentait d'arreter de suivre le fichier -- ce qui en pratique le
/// faisait REAPPARAITRE au prochain passage, un comportement juge plus
/// surprenant qu'une vraie suppression miroir).
pub fn synchroniser_dossier(
    client: &VexClient,
    dossier_local: &Path,
    dossier_distant_id: i64,
    prefixe_relatif: &str,
    etat: &mut EtatSync,
    log: &dyn Fn(&str),
) {
    let (sous_dossiers_distants, fichiers_distants) = match client.lister_dossier(dossier_distant_id) {
        Ok(v) => v,
        Err(e) => { log(&format!("Erreur (liste distante {prefixe_relatif}) : {e}")); return; }
    };

    let entrees_locales: Vec<PathBuf> = std::fs::read_dir(dossier_local)
        .map(|it| it.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    let fichiers_locaux: Vec<PathBuf> = entrees_locales.iter().filter(|p| p.is_file()).cloned().collect();
    let sous_dossiers_locaux: Vec<PathBuf> = entrees_locales.iter().filter(|p| p.is_dir()).cloned().collect();

    // ── Fichiers de ce niveau ────────────────────────────────────────
    let distants_par_nom: HashMap<String, &FileEntry> =
        fichiers_distants.iter().map(|f| (f.nom.clone(), f)).collect();
    let locaux_par_nom: HashMap<String, &PathBuf> = fichiers_locaux
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(|n| (n.to_string(), p)))
        .collect();

    let tous_les_noms: HashSet<&String> = distants_par_nom.keys().chain(locaux_par_nom.keys()).collect();
    for nom in tous_les_noms {
        let rel = if prefixe_relatif.is_empty() { nom.clone() } else { format!("{prefixe_relatif}/{nom}") };
        let distant = distants_par_nom.get(nom).copied();
        let local_path = locaux_par_nom.get(nom).copied();
        let baseline = etat.fichiers.get(&rel).cloned();

        match (local_path, distant, baseline) {
            // Nouveau seulement en local (jamais synchronise) -> upload.
            (Some(path), None, None) => {
                match std::fs::read(path) {
                    Ok(bytes) => match client.uploader_dans(dossier_distant_id, nom, &bytes, deviner_mime(nom)) {
                        Ok(id) => {
                            log(&format!("Envoye : {rel}"));
                            if let Ok(meta) = path.metadata() {
                                etat.fichiers.insert(rel.clone(), EtatFichier {
                                    remote_id: id, remote_date: String::new(),
                                    local_taille: meta.len(), local_mtime: mtime_secs(&meta),
                                });
                            }
                        }
                        Err(e) => log(&format!("Erreur envoi {rel} : {e}")),
                    },
                    Err(e) => log(&format!("Lecture impossible {rel} : {e}")),
                }
            }

            // Present seulement en local, MAIS deja synchronise -> le
            // fichier distant a ete supprime : on propage la suppression.
            (Some(_), None, Some(_)) => {
                match std::fs::remove_file(dossier_local.join(nom)) {
                    Ok(_) => { log(&format!("Supprime localement (supprime a distance) : {rel}")); etat.fichiers.remove(&rel); }
                    Err(e) => log(&format!("Suppression locale impossible {rel} : {e}")),
                }
            }

            // Nouveau seulement a distance (jamais synchronise) -> download.
            (None, Some(f), None) => {
                match client.telecharger(f.id) {
                    Ok(plain) => {
                        let dest = dossier_local.join(nom);
                        match std::fs::write(&dest, &plain) {
                            Ok(_) => {
                                log(&format!("Recu : {rel}"));
                                if let Ok(meta) = dest.metadata() {
                                    etat.fichiers.insert(rel.clone(), EtatFichier {
                                        remote_id: f.id, remote_date: f.date.clone(),
                                        local_taille: meta.len(), local_mtime: mtime_secs(&meta),
                                    });
                                }
                            }
                            Err(e) => log(&format!("Ecriture impossible {rel} : {e}")),
                        }
                    }
                    Err(e) => log(&format!("Erreur reception {rel} : {e}")),
                }
            }

            // Present seulement a distance, MAIS deja synchronise -> le
            // fichier local a ete supprime : on propage la suppression.
            (None, Some(f), Some(_)) => {
                match client.supprimer_fichier(f.id) {
                    Ok(_) => { log(&format!("Supprime a distance (supprime localement) : {rel}")); etat.fichiers.remove(&rel); }
                    Err(e) => log(&format!("Suppression distante impossible {rel} : {e}")),
                }
            }

            // Present des deux cotes, jamais vu par l'etat -> conflit prudent.
            (Some(path), Some(f), None) => {
                log(&format!("Conflit (jamais synchronise avant) : {rel} -- version locale conservee, version serveur enregistree a cote."));
                telecharger_copie_conflit(client, f, dossier_local, nom, &mut |m| log(m));
                if let Ok(meta) = path.metadata() {
                    etat.fichiers.insert(rel.clone(), EtatFichier {
                        remote_id: f.id, remote_date: f.date.clone(),
                        local_taille: meta.len(), local_mtime: mtime_secs(&meta),
                    });
                }
            }

            // Present des deux cotes, etat connu -> comparaison a la baseline.
            (Some(path), Some(f), Some(base)) => {
                let meta = path.metadata().ok();
                let local_change = meta.as_ref().map(|m| m.len() != base.local_taille || mtime_secs(m) != base.local_mtime).unwrap_or(false);
                let distant_change = f.date != base.remote_date;

                if local_change && distant_change {
                    log(&format!("Conflit (modifie des deux cotes) : {rel} -- version locale conservee, version serveur enregistree a cote."));
                    telecharger_copie_conflit(client, f, dossier_local, nom, &mut |m| log(m));
                    if let Some(m) = meta {
                        etat.fichiers.insert(rel.clone(), EtatFichier {
                            remote_id: f.id, remote_date: f.date.clone(),
                            local_taille: m.len(), local_mtime: mtime_secs(&m),
                        });
                    }
                } else if local_change {
                    match std::fs::read(path) {
                        Ok(bytes) => match client.remplacer_contenu(f.id, &bytes) {
                            Ok(_) => {
                                log(&format!("Envoye (mise a jour) : {rel}"));
                                if let Some(m) = meta {
                                    etat.fichiers.insert(rel.clone(), EtatFichier {
                                        remote_id: f.id, remote_date: base.remote_date.clone(),
                                        local_taille: m.len(), local_mtime: mtime_secs(&m),
                                    });
                                }
                            }
                            Err(e) => log(&format!("Erreur mise a jour {rel} : {e}")),
                        },
                        Err(e) => log(&format!("Lecture impossible {rel} : {e}")),
                    }
                } else if distant_change {
                    match client.telecharger(f.id) {
                        Ok(plain) => match std::fs::write(path, &plain) {
                            Ok(_) => {
                                log(&format!("Recu (mise a jour) : {rel}"));
                                if let Ok(m) = path.metadata() {
                                    etat.fichiers.insert(rel.clone(), EtatFichier {
                                        remote_id: f.id, remote_date: f.date.clone(),
                                        local_taille: m.len(), local_mtime: mtime_secs(&m),
                                    });
                                }
                            }
                            Err(e) => log(&format!("Ecriture impossible {rel} : {e}")),
                        },
                        Err(e) => log(&format!("Erreur reception {rel} : {e}")),
                    }
                }
                // Ni l'un ni l'autre n'a change : rien a faire.
            }

            // Connu de l'etat mais disparu d'un cote (ou des deux) : on
            // arrete de le suivre, sans jamais rien supprimer chez l'autre.
            (None, None, Some(_)) => {
                etat.fichiers.remove(&rel);
            }
            (None, None, None) => {}
        }
    }

    // ── Sous-dossiers : creation miroir puis recursion ──────────────
    let noms_dossiers_distants: HashSet<String> = sous_dossiers_distants.iter().map(|d| d.nom.clone()).collect();
    let noms_dossiers_locaux: HashMap<String, PathBuf> = sous_dossiers_locaux
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(|n| (n.to_string(), p.clone())))
        .collect();

    // Dossier distant sans equivalent local -> le creer localement.
    for d in &sous_dossiers_distants {
        if !noms_dossiers_locaux.contains_key(&d.nom) {
            let chemin = dossier_local.join(&d.nom);
            if let Err(e) = std::fs::create_dir_all(&chemin) {
                log(&format!("Impossible de creer le dossier local {} : {e}", chemin.display()));
                continue;
            }
        }
    }
    // Dossier local sans equivalent distant -> le creer a distance.
    let mut id_distant_par_nom: HashMap<String, i64> =
        sous_dossiers_distants.iter().map(|d| (d.nom.clone(), d.id)).collect();
    for (nom, _) in &noms_dossiers_locaux {
        if !id_distant_par_nom.contains_key(nom) {
            match client.creer_dossier(nom, dossier_distant_id) {
                Ok(id) => { id_distant_par_nom.insert(nom.clone(), id); }
                Err(e) => log(&format!("Impossible de creer le dossier distant {nom} : {e}")),
            }
        }
    }

    // Recursion dans chaque sous-dossier maintenant present des deux cotes.
    let tous_noms_dossiers: HashSet<String> = noms_dossiers_distants.into_iter().chain(noms_dossiers_locaux.keys().cloned()).collect();
    for nom in tous_noms_dossiers {
        let Some(&id) = id_distant_par_nom.get(&nom) else { continue };
        let chemin_local = dossier_local.join(&nom);
        if std::fs::create_dir_all(&chemin_local).is_err() { continue; }
        let rel = if prefixe_relatif.is_empty() { nom.clone() } else { format!("{prefixe_relatif}/{nom}") };
        synchroniser_dossier(client, &chemin_local, id, &rel, etat, log);
    }
}

/// Telecharge la version distante d'un fichier en conflit sous un nom de
/// copie a part, sans jamais toucher au fichier local existant.
fn telecharger_copie_conflit(client: &VexClient, f: &FileEntry, dossier_local: &Path, nom: &str, log: &mut dyn FnMut(&str)) {
    match client.telecharger(f.id) {
        Ok(plain) => {
            let copie = nom_copie_conflit(nom);
            match std::fs::write(dossier_local.join(&copie), &plain) {
                Ok(_) => log(&format!("  -> version serveur enregistree sous : {copie}")),
                Err(e) => log(&format!("  -> impossible d'ecrire la copie de conflit {copie} : {e}")),
            }
        }
        Err(e) => log(&format!("  -> impossible de recuperer la version serveur pour comparaison : {e}")),
    }
}
