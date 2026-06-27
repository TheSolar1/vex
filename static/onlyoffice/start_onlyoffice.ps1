# Démarrage à la demande du service OnlyOffice DocumentServer sur Windows.
# Nécessite que l'installateur officiel OnlyOffice DocumentServer soit déjà installé.
# Ajuste si besoin les noms de service ci-dessous.

$serviceCandidates = @(
    "ONLYOFFICE DocumentServer",
    "OnlyOfficeDocumentServer",
    "onlyoffice-documentserver",
    "ONLYOFFICE Document Server"
)

$found = $false
foreach ($name in $serviceCandidates) {
    try {
        $svc = Get-Service -Name $name -ErrorAction Stop
        $found = $true
        if ($svc.Status -ne 'Running') {
            Start-Service -Name $name
            $svc.WaitForStatus('Running','00:00:20')
        }
        Write-Output "Service '$name' démarré. Statut: $((Get-Service -Name $name).Status)"
        break
    } catch {
        continue
    }
}

if (-not $found) {
    Write-Error "Service OnlyOffice DocumentServer introuvable. Installe le package Windows officiel puis ajuste start_onlyoffice.ps1."
    exit 1
}
