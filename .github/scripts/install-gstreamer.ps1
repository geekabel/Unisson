# Installs GStreamer on a Windows runner, and tells the next steps where it
# is: the build finds its libraries through pkg-config.
$ErrorActionPreference = "Stop"

choco install -y --no-progress gstreamer gstreamer-devel
if ($LASTEXITCODE -ne 0) { throw "Chocolatey could not install GStreamer" }

# The installer records its folder in the settings of the machine. It picks
# the drive itself, so the folder is read back instead of being guessed.
$root = [Environment]::GetEnvironmentVariable("GSTREAMER_1_0_ROOT_MSVC_X86_64", "Machine")
if (-not $root) { throw "GStreamer did not record its folder" }
$root = $root.TrimEnd("\")
Write-Host "GStreamer is in $root"

"GSTREAMER_1_0_ROOT_MSVC_X86_64=$root\" >> $env:GITHUB_ENV
"PKG_CONFIG_PATH=$root\lib\pkgconfig" >> $env:GITHUB_ENV
"$root\bin" >> $env:GITHUB_PATH
