# Installs powermise, the unofficial desktop companion for mise, on Windows
# from its GitHub releases:
#
#   irm https://powermise.dev/install.ps1 | iex
#
# Runs the installer quietly for your account only, so it doesn't ask for
# administrator rights. Settings, both optional:
#   $env:POWERMISE_VERSION = '0.2.0'      a specific release (default: the latest)
#   $env:POWERMISE_DOWNLOAD_URL = '<url>' a mirror of the release files
$ErrorActionPreference = 'Stop'

$repo = 'frankievalentine/powermise'
$version = if ($env:POWERMISE_VERSION) { $env:POWERMISE_VERSION.TrimStart('v') } else { 'latest' }
$base = if ($env:POWERMISE_DOWNLOAD_URL) { $env:POWERMISE_DOWNLOAD_URL.TrimEnd('/') }
elseif ($version -eq 'latest') { "https://github.com/$repo/releases/latest/download" }
else { "https://github.com/$repo/releases/download/v$version" }

if (-not [Environment]::Is64BitOperatingSystem) {
  throw 'powermise needs 64-bit Windows.'
}

$file = 'powermise-windows-x64-setup.exe'
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("powermise-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  # The progress bar slows Invoke-WebRequest down a lot in Windows PowerShell.
  $ProgressPreference = 'SilentlyContinue'
  Write-Host "Downloading $file..."
  Invoke-WebRequest -UseBasicParsing -Uri "$base/$file" -OutFile (Join-Path $tmp $file)
  Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS')

  $line = Get-Content (Join-Path $tmp 'SHA256SUMS') | Where-Object { ($_ -split '\s+')[1] -eq $file } | Select-Object -First 1
  if (-not $line) { throw "The release has no checksum for $file." }
  $expected = ($line -split '\s+')[0]
  $actual = (Get-FileHash (Join-Path $tmp $file) -Algorithm SHA256).Hash
  if ($actual -ne $expected) { throw "$file doesn't match its checksum; nothing was installed." }

  Write-Host 'Installing powermise...'
  $installer = Start-Process -FilePath (Join-Path $tmp $file) -ArgumentList '/S' -Wait -PassThru
  if ($installer.ExitCode -ne 0) { throw "The installer stopped with exit code $($installer.ExitCode)." }

  Write-Host 'Installed powermise. Open it from the Start menu.'
  if (-not (Get-Command mise -ErrorAction SilentlyContinue)) {
    Write-Host 'powermise runs mise for everything it does; it offers to install mise the first time it opens.'
  }
}
finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
