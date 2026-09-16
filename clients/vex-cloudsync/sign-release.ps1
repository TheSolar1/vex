# Signe vex-cloudsync.exe et desinstaller.exe de facon reproductible : meme
# certificat + meme serveur d'horodatage a chaque build, pour que la
# reputation SmartScreen / Safe Browsing s'accumule au lieu de repartir de
# zero a chaque version.
#
# A REMPLIR avant utilisation :
#   - $CertPath : chemin vers ton .pfx (laisse vide si le cert est deja
#     installe dans le magasin Windows -- utilise alors $CertThumbprint)
#   - $CertThumbprint : empreinte du certificat dans Cert:\CurrentUser\My
#     (visible via `certmgr.msc` ou `Get-ChildItem Cert:\CurrentUser\My`)
#
# Usage (rapide, apres avoir rempli $CertThumbprint ci-dessous une fois) :
#   .\sign-release.ps1 -Deploy
# Ca build en release, signe vex-cloudsync.exe ET desinstaller.exe, et les
# copie vers ..\..\static\downloads\ (fichiers servis par le serveur --
# voir CHEMIN_EXE_CLOUDSYNC / CHEMIN_DESINSTALLER dans src\login\appareil.rs).
#
# Usage explicite (un seul fichier, chemin/cert au cas par cas) :
#   .\sign-release.ps1 -ExePath ..\..\static\downloads\vex-cloudsync.exe -CertThumbprint A1B2...

param(
    [string]$ExePath,
    [string]$CertPath,
    [string]$CertThumbprint,
    [switch]$Deploy
)

if (-not $CertPath) { $CertPath = "" }
if (-not $CertThumbprint) { $CertThumbprint = "C64B53726CEEA315DBE39FAF0747E9DF9674ADB8" } # CN=VEX Corp by TheSolar
$TimestampUrl = "http://timestamp.digicert.com"

function Sign-Exe {
    param([string]$Chemin)

    if (-not (Test-Path $Chemin)) {
        Write-Error "Fichier introuvable : $Chemin"
        exit 1
    }

    if ($CertPath -ne "") {
        $securePwd = Read-Host -Prompt "Mot de passe du .pfx" -AsSecureString
        Set-AuthenticodeSignature -FilePath $Chemin `
            -Certificate (Get-PfxCertificate -FilePath $CertPath) `
            -TimestampServer $TimestampUrl `
            -HashAlgorithm SHA256 | Out-Null
    }
    elseif ($CertThumbprint -ne "") {
        $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Thumbprint -eq $CertThumbprint }
        if (-not $cert) {
            Write-Error "Certificat introuvable pour le thumbprint $CertThumbprint dans Cert:\CurrentUser\My"
            exit 1
        }
        Set-AuthenticodeSignature -FilePath $Chemin `
            -Certificate $cert `
            -TimestampServer $TimestampUrl `
            -HashAlgorithm SHA256 | Out-Null
    }
    else {
        Write-Error "Remplis CertPath ou CertThumbprint en haut du script avant de l'utiliser."
        exit 1
    }

    $sig = Get-AuthenticodeSignature -FilePath $Chemin
    if ($sig.Status -eq "Valid") {
        Write-Host "OK -- $Chemin signe par : $($sig.SignerCertificate.Subject)" -ForegroundColor Green
        Write-Host "Hash SHA256 : $((Get-FileHash $Chemin -Algorithm SHA256).Hash)"
    } else {
        Write-Error "Signature invalide ($Chemin) : $($sig.StatusMessage)"
        exit 1
    }
}

if ($Deploy) {
    Write-Host "Build release..." -ForegroundColor Cyan
    Push-Location $PSScriptRoot
    cargo build --release --bin vex-cloudsync --bin desinstaller
    $buildOk = $LASTEXITCODE -eq 0
    Pop-Location
    if (-not $buildOk) {
        Write-Error "cargo build --release a echoue."
        exit 1
    }

    $exeCloudsync = Join-Path $PSScriptRoot "target\release\vex-cloudsync.exe"
    $exeDesinstaller = Join-Path $PSScriptRoot "target\release\desinstaller.exe"
    Sign-Exe -Chemin $exeCloudsync
    Sign-Exe -Chemin $exeDesinstaller

    $targetCloudsync = Join-Path $PSScriptRoot "..\..\static\downloads\vex-cloudsync.exe"
    $targetDesinstaller = Join-Path $PSScriptRoot "..\..\static\downloads\desinstaller.exe"
    Copy-Item -Path $exeCloudsync -Destination $targetCloudsync -Force
    Copy-Item -Path $exeDesinstaller -Destination $targetDesinstaller -Force
    Write-Host "Deploye vers $targetCloudsync" -ForegroundColor Green
    Write-Host "Deploye vers $targetDesinstaller" -ForegroundColor Green

    # Le zip (exe + desinstaller.exe + notice traduite + langue.txt) n'est
    # pas pre-construit ici : le serveur le genere a la volee par
    # utilisateur, dans SA langue de compte (voir
    # src/login/appareil.rs::telecharger_bundle et notice_cloudsync.rs).
    # Seuls les deux exe signes doivent etre a jour sur le disque.
}
elseif ($ExePath) {
    Sign-Exe -Chemin $ExePath
}
else {
    Write-Error "Precise -ExePath, ou utilise -Deploy pour build+signer+deployer les deux exe en une commande."
    exit 1
}
