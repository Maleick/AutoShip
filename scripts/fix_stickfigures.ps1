$iniPath = "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest\eqclient.ini"
$lines = Get-Content $iniPath

# Remove any existing StickFigures line (wherever it ended up)
$lines = $lines | Where-Object { $_ -notmatch "^StickFigures=" }

# Insert after [Defaults] line
$output = @()
foreach ($line in $lines) {
    $output += $line
    if ($line -match "^\[Defaults\]") {
        $output += "StickFigures=1"
    }
}

Set-Content $iniPath $output
Write-Host "StickFigures=1 inserted into [Defaults] section"
Select-String -Path $iniPath -Pattern "StickFigures"
