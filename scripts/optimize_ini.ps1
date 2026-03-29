$iniPath = "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest\eqclient.ini"

# Backup
Copy-Item $iniPath "$iniPath.bak" -Force

$content = Get-Content $iniPath -Raw

# Ensure these settings exist with correct values
$settings = @{
    "StickFigures" = "1"
    "Shadows" = "0"
    "MaxParticles" = "0"
    "ShowGrass" = "0"
    "MaxBGFPS" = "10"
}

foreach ($key in $settings.Keys) {
    $val = $settings[$key]
    if ($content -match "(?m)^$key=") {
        $content = $content -replace "(?m)^$key=.*", "$key=$val"
    } else {
        # Insert after [Defaults]
        $content = $content -replace "(\[Defaults\])", "`$1`r`n$key=$val"
    }
}

# Clean up any broken appended lines at end of file
$content = $content -replace "(?m)^Shadows= \s*$", ""
$content = $content -replace "(?m)^MaxParticles= \s*$", ""
$content = $content -replace "(?m)^StickFigures=\s*$", ""

Set-Content $iniPath $content -NoNewline

Write-Host "INI optimized. Verifying key settings:"
Select-String -Path $iniPath -Pattern "StickFigures|Shadows|MaxBGFPS|MaxFPS|Sound=|AllLuclinPcModels|MaxParticles|ShowGrass"
