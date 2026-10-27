# Installs Figghost for Windows from the latest GitHub release.
#
#   irm https://raw.githubusercontent.com/eliasbui/figghost/main/scripts/install.ps1 | iex
#   $env:DEST = "$env:LOCALAPPDATA\Programs\Figghost"; irm https://raw.githubusercontent.com/eliasbui/figghost/main/scripts/install.ps1 | iex
#
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = "eliasbui/figghost"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "install: Windows only"
}
if (-not [Environment]::Is64BitOperatingSystem) {
    throw "install: 64-bit Windows required"
}

# Enable TLS 1.2 for PowerShell 5.1
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$LocalAppData = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA } else { Join-Path $env:USERPROFILE "AppData\Local" }
$Dest = if ($env:DEST) { $env:DEST } else { Join-Path $LocalAppData "Programs\Figghost" }
$Dest = $Dest.TrimEnd('\')
$Exe = Join-Path $Dest "figghost.exe"

$Headers = @{ "User-Agent" = "figghost-installer" }
$Version = $env:VERSION
$DownloadUrl = $null

if (-not $Version) {
    try {
        $Releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases" -Headers $Headers -UseBasicParsing
        foreach ($Rel in $Releases) {
            $Asset = $Rel.assets | Where-Object { $_.name -like "*windows-x86_64.zip" -or $_.name -like "*windows*.zip" } | Select-Object -First 1
            if ($Asset) {
                $Version = $Rel.tag_name.TrimStart('v')
                $DownloadUrl = $Asset.browser_download_url
                break
            }
        }
    } catch {
        # Fall back to latest tag query
    }
}

if (-not $Version) {
    try {
        $Latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers $Headers -UseBasicParsing
        $Version = $Latest.tag_name.TrimStart('v')
        $Asset = $Latest.assets | Where-Object { $_.name -like "*windows-x86_64.zip" -or $_.name -like "*windows*.zip" } | Select-Object -First 1
        if ($Asset) {
            $DownloadUrl = $Asset.browser_download_url
        }
    } catch {
        throw "install: could not determine latest release version from GitHub API"
    }
}

if (-not $Version) {
    throw "install: no release found for $Repo"
}

if (-not $DownloadUrl) {
    $DownloadUrl = "https://github.com/$Repo/releases/download/v$Version/figghost-$Version-windows-x86_64.zip"
}

$Work = Join-Path ([System.IO.Path]::GetTempPath()) "figghost-install-$(Get-Random)"
New-Item -ItemType Directory -Path $Work -Force | Out-Null
$Zip = Join-Path $Work "figghost-$Version.zip"

try {
    Write-Host "install: Figghost $Version"
    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $Zip -UseBasicParsing
    } catch {
        throw "install: failed to download $DownloadUrl ($($_.Exception.Message))"
    }

    $ExtractDir = Join-Path $Work "extracted"
    Expand-Archive -Path $Zip -DestinationPath $ExtractDir -Force

    $FoundExe = Get-ChildItem -Path $ExtractDir -Filter "figghost.exe" -Recurse | Select-Object -First 1
    if (-not $FoundExe) {
        throw "install: the download does not contain figghost.exe"
    }

    # Stop running instance if upgrading
    Get-Process -Name "figghost" -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 200

    if (-not (Test-Path $Dest)) {
        New-Item -ItemType Directory -Path $Dest -Force | Out-Null
    }

    $SourceDir = $FoundExe.DirectoryName
    Get-ChildItem -Path $SourceDir | Copy-Item -Destination $Dest -Recurse -Force

    # Remove mark of the web
    Get-ChildItem -Path $Dest -Recurse | Unblock-File -ErrorAction SilentlyContinue

    # Update User PATH
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $Paths = if ($UserPath) { ($UserPath -split ';') | ForEach-Object { $_.Trim().TrimEnd('\') } | Where-Object { $_ } } else { @() }
    if ($Paths -notcontains $Dest) {
        $NewPath = if ($UserPath -and $UserPath.Trim()) { "$UserPath;$Dest" } else { $Dest }
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    }
    $env:Path = "$Dest;$env:Path"

    # Start Menu shortcut
    $StartMenu = [Environment]::GetFolderPath("Programs")
    if ($StartMenu -and (Test-Path $StartMenu)) {
        try {
            $Wsh = New-Object -ComObject WScript.Shell
            $Shortcut = $Wsh.CreateShortcut((Join-Path $StartMenu "Figghost.lnk"))
            $Shortcut.TargetPath = $Exe
            $Shortcut.WorkingDirectory = $Dest
            $Shortcut.Description = "Figghost"
            $Shortcut.Save()
        } catch {
            # Best-effort
        }
    }

    Write-Host "install: $Exe"
    Write-Host "install: run with 'figghost' or launch from the Start Menu"
} finally {
    Remove-Item -Path $Work -Recurse -Force -ErrorAction SilentlyContinue
}
