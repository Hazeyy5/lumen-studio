$ErrorActionPreference = "Stop"
$root = Join-Path $env:LOCALAPPDATA "Lumen\tools"
New-Item -ItemType Directory -Force -Path $root | Out-Null

function Get-ZipUrl($repo, $test) {
  $release = Invoke-RestMethod -Headers @{ "User-Agent" = "Lumen" } -Uri "https://api.github.com/repos/$repo/releases/latest"
  $asset = $release.assets | Where-Object { & $test $_.name } | Select-Object -First 1
  if (-not $asset) { throw "Binaire introuvable pour $repo" }
  return $asset.browser_download_url
}

function Install-Zip($repo, $dest, $test, $exe) {
  if (Get-Command $exe -ErrorAction SilentlyContinue) { return }
  if (Test-Path $dest) {
    $found = Get-ChildItem -Path $dest -Filter "$exe.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) { return }
  }
  $url = Get-ZipUrl $repo $test
  $zip = Join-Path $env:TEMP "$(Split-Path $dest -Leaf).zip"
  Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
  if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
  New-Item -ItemType Directory -Force -Path $dest | Out-Null
  Expand-Archive -Path $zip -DestinationPath $dest -Force
}

Install-Zip "git-for-windows/git" (Join-Path $root "mingit") {
  param($name)
  $lower = $name.ToLower()
  $lower.StartsWith("mingit-") -and $lower.Contains("64-bit") -and $lower.EndsWith(".zip") -and -not $lower.Contains("busybox") -and -not $lower.Contains("arm64")
} "git"

Install-Zip "cli/cli" (Join-Path $root "gh") {
  param($name)
  $lower = $name.ToLower()
  $lower.StartsWith("gh_") -and $lower.Contains("windows_amd64") -and $lower.EndsWith(".zip")
} "gh"
