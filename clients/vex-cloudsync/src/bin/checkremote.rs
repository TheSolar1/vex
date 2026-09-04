use vex_sync_client::api::VexClient;

fn main() {
    let base_url = std::env::args().nth(1).unwrap();
    let email = std::env::args().nth(2).unwrap();
    let password = std::env::args().nth(3).unwrap();
    let client = VexClient::login(&base_url, &email, &password).expect("login");
    let (dossiers, fichiers) = client.lister_dossier(0).expect("liste racine");
    println!("Dossiers a la racine :");
    for d in &dossiers { println!("  [{}] {}", d.id, d.nom); }
    println!("Fichiers a la racine :");
    for f in &fichiers { println!("  [{}] {} ({} octets)", f.id, f.nom, f.taille); }
    for d in &dossiers {
        let (_, fichiers_sous) = client.lister_dossier(d.id).expect("liste sous-dossier");
        println!("Fichiers dans {} :", d.nom);
        for f in &fichiers_sous { println!("  [{}] {} ({} octets)", f.id, f.nom, f.taille); }
    }
}
