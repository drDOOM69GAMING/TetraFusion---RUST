$ErrorActionPreference = "Stop"

# Uploads the release build to a GitHub release as an asset, replacing any
# existing one of the same name.
#
# This exists because the same upload failed three different ways by hand:
# PowerShell's `-Uri` rejected the templated `upload_url` that the releases API
# hands back (`.../assets{?name,label}`), so the fix is to strip the template
# segment and the trailing `?` before the URI ever reaches the cmdlet. Getting
# that wrong is why the release shipped with no exe on it while the release page
# said nothing was wrong.
#
# Usage:  $env:GH_TOKEN = "..."   then   powershell -File tools\upload-release.ps1 [tag]

$token = $env:GH_TOKEN
if (-not $token) { throw "Set GH_TOKEN first: `$env:GH_TOKEN = 'ghp_...'" }

$owner = "drDOOM69GAMING"
$repo = "TetraFusion---RUST"
$tag = if ($args.Count -gt 0) { $args[0] } else { "v2.2.0" }
$exe = if ($args.Count -gt 2) { $args[2] } else { Join-Path $PSScriptRoot "..\target\release\tetrafusion.exe" }
# The shipped name carries the version and the "-packed" suffix, so the
# download tells you what it is without anyone having to open the page.
$name = if ($args.Count -gt 1) { $args[1] } else { "TetraFusion-$($tag -replace '^v','')-packed.exe" }

if (-not (Test-Path $exe)) { throw "Build it first: cargo build --release" }

$apiHeaders = @{
    Authorization = "Bearer $token"
    Accept = "application/vnd.github+json"
    "X-GitHub-Api-Version" = "2022-11-28"
    "User-Agent" = "tetrafusion-release"
}

$rel = Invoke-RestMethod -Uri "https://api.github.com/repos/$owner/$repo/releases/tags/$tag" -Headers $apiHeaders -Method Get
$upload = [string]$rel.upload_url

# The asset API rejects a duplicate name, so an existing one has to go first.
foreach ($a in @($rel.assets)) {
    if ($a.name -eq $name) {
        Write-Host "replacing existing asset $($a.name)"
        Invoke-RestMethod -Uri "https://api.github.com/repos/$owner/$repo/releases/assets/$($a.id)" -Headers $apiHeaders -Method Delete
    }
}

# `upload_url` comes back as `https://uploads.github.com/.../assets{?name,label}`.
# The `{...}` is a URI *template*, not part of the address, and PowerShell's -Uri
# reports "The hostname could not be parsed" when it is left in.
#
# The parse is done with `IndexOf`/`Substring` and the query is concatenated
# rather than interpolated on purpose. `"$base?name=x"` is not safe here: PowerShell
# treats `?` after an interpolated value as part of the *next* expression, and
# silently produces `=x` - which is a URI with no host and no parameter name.
$brace = $upload.IndexOf('{')
if ($brace -ge 0) { $upload = $upload.Substring(0, $brace) }
$upload = $upload.TrimEnd([char[]]@('?', ' '))
if ($upload -notmatch '^https://') { throw "unexpected upload_url: [$upload]" }
$uri = $upload + '?name=' + $name

# Prove it parses as a real URI before spending an upload on it.
$parsed = [uri]$uri
if ($parsed.Host -ne 'uploads.github.com') { throw "upload URI host is '$($parsed.Host)'" }
Write-Host "upload URI: $uri"

Write-Host "uploading $name ($([math]::Round((Get-Item $exe).Length/1MB,2)) MB) to $uri"

$uploaded = Invoke-RestMethod -Uri $uri -Method Post -Headers @{
    Authorization = "Bearer $token"
    Accept = "application/vnd.github+json"
    "X-GitHub-Api-Version" = "2022-11-28"
    "User-Agent" = "tetrafusion-release"
} -ContentType "application/octet-stream" -InFile $exe

Write-Host "uploaded: $($uploaded.browser_download_url)"
$uploaded | Select-Object name, size, browser_download_url | Format-List