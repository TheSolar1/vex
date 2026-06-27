# Arrêt à la demande du service OnlyOffice DocumentServer sur Windows.
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
        if ($svc.Status -ne 'Stopped') {
            Stop-Service -Name $name -Force
            $svc.WaitForStatus('Stopped','00:00:20')
        }
        Write-Output "Service '$name' arrêté. Statut: $((Get-Service -Name $name).Status)"
        break
    } catch {
        continue
    }
}

if (-not $found) {
    Write-Error "Service OnlyOffice DocumentServer introuvable. Vérifie le nom ou installe le package Windows officiel."
    exit 1
}
