$ErrorActionPreference = "Stop"

$repository = "autotntfan/envfind-cli"
$version = "latest"
if ($version -ne "latest" -and $version -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$') {
    throw "Installer version must be a release tag such as v0.1.0"
}

$releaseBaseUri = if ($version -eq "latest") {
    "https://github.com/$repository/releases/latest/download"
} else {
    "https://github.com/$repository/releases/download/$version"
}
$assetUri = "$releaseBaseUri/envfind-x86_64-pc-windows-msvc.exe"
$checksumUri = "$releaseBaseUri/SHA256SUMS"

$tempFile = Join-Path ([IO.Path]::GetTempPath()) "envfind-$([guid]::NewGuid()).exe"
$checksumFile = Join-Path ([IO.Path]::GetTempPath()) "envfind-$([guid]::NewGuid()).sha256"
try {
    Invoke-WebRequest -Uri $assetUri -OutFile $tempFile
    Invoke-WebRequest -Uri $checksumUri -OutFile $checksumFile

    $binaryBytes = [IO.File]::ReadAllBytes($tempFile)
    $peHeaderOffset = if ($binaryBytes.Length -ge 64) {
        [BitConverter]::ToInt32($binaryBytes, 0x3c)
    } else {
        -1
    }
    $validPe = $binaryBytes.Length -ge 64 -and
        $binaryBytes[0] -eq 0x4d -and
        $binaryBytes[1] -eq 0x5a -and
        $peHeaderOffset -ge 64 -and
        $peHeaderOffset -le ($binaryBytes.Length - 4)
    if ($validPe) {
        $validPe = $binaryBytes[$peHeaderOffset] -eq 0x50 -and
            $binaryBytes[$peHeaderOffset + 1] -eq 0x45 -and
            $binaryBytes[$peHeaderOffset + 2] -eq 0 -and
            $binaryBytes[$peHeaderOffset + 3] -eq 0
    }
    if ($validPe) {
        $machineOffset = $peHeaderOffset + 4
        $optionalHeaderOffset = $peHeaderOffset + 24
        $validPe = $machineOffset + 2 -le $binaryBytes.Length -and
            $optionalHeaderOffset + 2 -le $binaryBytes.Length -and
            [BitConverter]::ToUInt16($binaryBytes, $machineOffset) -eq 0x8664 -and
            [BitConverter]::ToUInt16($binaryBytes, $optionalHeaderOffset) -eq 0x20b
    }
    if (-not $validPe) {
        throw "PE header validation failed"
    }

    $checksumPattern = '^\s*([0-9A-Fa-f]{64})\s+\*?envfind-x86_64-pc-windows-msvc\.exe\s*$'
    $matchingLines = @(Get-Content -LiteralPath $checksumFile | Where-Object { $_ -match $checksumPattern })
    if ($matchingLines.Count -ne 1) {
        throw "SHA256SUMS does not contain exactly one Windows binary entry"
    }
    $expected = [regex]::Match($matchingLines[0], $checksumPattern).Groups[1].Value
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $tempFile).Hash.ToLowerInvariant()
    if (-not $expected -or $actual -ne $expected.ToLowerInvariant()) {
        throw "SHA256 verification failed"
    }

    $installDirectory = Join-Path $env:LOCALAPPDATA "envfind\bin"
    New-Item -ItemType Directory -Force -Path $installDirectory | Out-Null
    Copy-Item -LiteralPath $tempFile -Destination (Join-Path $installDirectory "envfind.exe") -Force

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $entries = @($userPath -split ";" | Where-Object { $_ })
    if ($entries -notcontains $installDirectory) {
        [Environment]::SetEnvironmentVariable("Path", (($entries + $installDirectory) -join ";"), "User")
    }
    Write-Host "Installed envfind to $installDirectory\envfind.exe"
    Write-Host "Open a new terminal, then run: envfind --help"
} finally {
    Remove-Item -LiteralPath $tempFile, $checksumFile -Force -ErrorAction SilentlyContinue
}
