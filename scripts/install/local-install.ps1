<#
.SYNOPSIS
Compile openagere from source and replace the npm-managed binary on Windows.

.DESCRIPTION
Builds the openagere binary and replaces the binary inside an existing npm
installation's platform package so the npm-managed `openagere` shim runs the
locally built binary.

.PARAMETER Uninstall
If set, removes the old standalone local binary created by earlier versions of
this script.

.EXAMPLE
.\scripts\install\local-install.ps1
    Build a release binary and replace the binary used by the npm installation.

.EXAMPLE
.\scripts\install\local-install.ps1 -Uninstall
    Remove the old standalone local openagere binary.
#>

param(
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($env:OPENAGERE_INSTALL_DIR)) {
    $installDir = Join-Path $env:LOCALAPPDATA 'Programs\openagere\bin'
} else {
    $installDir = $env:OPENAGERE_INSTALL_DIR
}
$binPath = Join-Path $installDir 'openagere.exe'
$targetTripleByArch = @{
    AMD64 = 'x86_64-pc-windows-msvc'
    ARM64 = 'aarch64-pc-windows-msvc'
}

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message"
}

function Uninstall-Binary {
    if (Test-Path -LiteralPath $binPath) {
        Write-Step "Removing $binPath"
        Remove-Item -LiteralPath $binPath -Force
    } else {
        Write-Step "No installation found at $binPath"
    }
}

function Get-RepoRoot {
    $repoRoot = Join-Path $PSScriptRoot '..\..'
    (Resolve-Path $repoRoot).Path
}

function Build-OpenAgere {
    param([string]$RepoRoot)

    # Use parallel codegen units for faster compilation
    $cores = (Get-CimInstance Win32_Processor | Measure-Object -Property NumberOfLogicalProcessors -Sum).Sum
    if ($cores -gt 0) {
        $env:CARGO_BUILD_JOBS = [string]$cores
    }

    $profile = 'release'
    Write-Step "Building openagere with $profile profile ($cores parallel jobs)..."
    Push-Location $RepoRoot
    try {
        cargo build --release -p agere-cli --bin openagere
    } finally {
        Pop-Location
    }

    $binaryPath = Join-Path $RepoRoot "target\$profile\openagere.exe"
    if (-not (Test-Path -LiteralPath $binaryPath)) {
        throw 'Build succeeded but binary not found at ' + $binaryPath
    }

    $binaryPath
}

function Get-NpmPackageRoot {
    $npmRoot = (& npm root -g 2>$null | Select-Object -First 1)
    if (-not [string]::IsNullOrWhiteSpace($npmRoot)) {
        $candidate = Join-Path $npmRoot 'openagere'
        if (Test-Path -LiteralPath (Join-Path $candidate 'package.json')) {
            return (Resolve-Path $candidate).Path
        }
    }

    $openagereCommand = Get-Command openagere -ErrorAction SilentlyContinue
    if ($openagereCommand) {
        $commandPath = $openagereCommand.Source
        if ($commandPath) {
            $commandDir = Split-Path -Parent $commandPath
            $direct = Join-Path $commandDir 'node_modules\openagere'
            if (Test-Path -LiteralPath (Join-Path $direct 'package.json')) {
                return (Resolve-Path $direct).Path
            }
            $nvm = Join-Path (Split-Path -Parent $commandDir) 'lib\node_modules\openagere'
            if (Test-Path -LiteralPath (Join-Path $nvm 'package.json')) {
                return (Resolve-Path $nvm).Path
            }
        }
    }

    throw 'Could not find a global npm installation of openagere. Run `npm install -g openagere` first, or set OPENAGERE_NPM_PACKAGE_DIR.'
}

function Get-NpmVendorBinaryPath {
    if ($env:OPENAGERE_NPM_BINARY) {
        return $env:OPENAGERE_NPM_BINARY
    }

    $packageRoot = if ($env:OPENAGERE_NPM_PACKAGE_DIR) {
        $env:OPENAGERE_NPM_PACKAGE_DIR
    } else {
        Get-NpmPackageRoot
    }

    if (-not $targetTripleByArch.ContainsKey($env:PROCESSOR_ARCHITECTURE)) {
        throw "Unsupported Windows architecture for npm binary replacement: $env:PROCESSOR_ARCHITECTURE"
    }
    $targetTriple = $targetTripleByArch[$env:PROCESSOR_ARCHITECTURE]
    $platformPackageByTarget = @{
        'x86_64-pc-windows-msvc' = '@openagere\openagere-win32-x64'
        'aarch64-pc-windows-msvc' = '@openagere\openagere-win32-arm64'
    }
    $platformPackagePath = Join-Path (Split-Path -Parent $packageRoot) $platformPackageByTarget[$targetTriple]
    $binaryPath = Join-Path $platformPackagePath "vendor\$targetTriple\bin\openagere.exe"

    if (-not (Test-Path -LiteralPath $binaryPath)) {
        $nestedPlatformPackagePath = Join-Path $packageRoot "node_modules\$($platformPackageByTarget[$targetTriple])"
        $nestedBinaryPath = Join-Path $nestedPlatformPackagePath "vendor\$targetTriple\bin\openagere.exe"
        if (Test-Path -LiteralPath $nestedBinaryPath) {
            return $nestedBinaryPath
        }
        $localBinaryPath = Join-Path $packageRoot "vendor\$targetTriple\bin\openagere.exe"
        if (Test-Path -LiteralPath $localBinaryPath) {
            return $localBinaryPath
        }
        throw "Could not find npm vendor binary. Checked: $binaryPath, $nestedBinaryPath, and $localBinaryPath"
    }

    $binaryPath
}

function Install-Binary {
    $repoRoot = Get-RepoRoot
    $binaryPath = Build-OpenAgere -RepoRoot $repoRoot
    $npmBinaryPath = Get-NpmVendorBinaryPath

    Write-Step "Replacing npm vendor binary at $npmBinaryPath..."
    Copy-Item -LiteralPath $binaryPath -Destination $npmBinaryPath -Force

    Write-Step 'Done!'
    Write-Host ''
    Write-Host "npm-managed openagere now uses $binaryPath"
    Write-Host 'Run: openagere --version'
}

if ($Uninstall) {
    Uninstall-Binary
} else {
    Install-Binary
}
