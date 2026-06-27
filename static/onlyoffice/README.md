# OnlyOffice Server – intégration VEX

Ce dossier sert à documenter l’intégration du Document Server OnlyOffice côté VEX (statique, aucune binaire ici).

## Où régler OnlyOffice ?
- `config.json`
  - `extensions.extension_params.onlyoffice` : options d’édition (auto_save, jwt_enabled, jwt_secret, tailles, formats).
  - `onlyoffice_server` : contrôle du serveur (server_url, healthcheck_path, start_cmd, stop_cmd, wait_boot_ms, auto_stop).
- `src/config_loader.rs` : structs `EditorConfig` et `OnlyofficeServerConfig` qui lisent ces blocs.
- `src/admin/Admin.rs` : API admin
  - `GET /api/admin/onlyoffice` : état (online, ping ms, params).
  - `POST /api/admin/onlyoffice/start` : lance le serveur via `start_cmd`.
  - `POST /api/admin/onlyoffice/stop` : l’arrête via `stop_cmd`.

## Exemple de commandes (à adapter)
- Démarrer (docker) : `docker run -d --name vex-onlyoffice -p 8080:80 onlyoffice/documentserver:latest`
- Arrêter : `docker stop vex-onlyoffice && docker rm vex-onlyoffice`

## À personnaliser
1. Mettre un vrai secret dans `extensions.extension_params.onlyoffice.jwt_secret` si `jwt_enabled=true`.
2. Adapter `start_cmd` / `stop_cmd` à ton environnement (docker, systemd, service Windows, etc.).
3. Ajuster `server_url` et `healthcheck_path` si le port ou le contexte change.
