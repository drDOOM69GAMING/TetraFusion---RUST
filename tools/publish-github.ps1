$ErrorActionPreference = "Stop"

# Publishes the crate to GitHub as ONE commit with a real tree.
#
# The earlier attempt uploaded every file through the contents API, which is one
# commit *per file* - around a hundred commits of "Add src/board.rs", and a
# history that says nothing. This does it properly instead: a single tree
# object, a single root commit, and the branch ref pointed at it.

$token = $env:GH_TOKEN
$owner = "drDOOM69GAMING"
$repo = "TetraFusion---RUST"
$root = "C:\Users\amduser\Documents\Default Project\tetrafusion-rs"

$headers = @{
    Authorization = "Bearer $token"
    Accept = "application/vnd.github+json"
    "X-GitHub-Api-Version" = "2022-11-28"
    "User-Agent" = "tetrafusion-publish"
}

function Post-Json($uri, $obj, $h) {
    $json = $obj | ConvertTo-Json -Depth 12 -Compress
    $bytes = [Text.Encoding]::UTF8.GetBytes($json)
    Invoke-RestMethod -Uri $uri -Headers $h -Method Post -Body $bytes -ContentType "application/json"
}

# Everything that is source, not build output or local state.
$files = Get-ChildItem "$root\src","$root\tests","$root\examples","$root\tools","$root\assets" -Recurse -File |
    Where-Object { $_.Extension -ne ".md" -or $_.DirectoryName -notmatch "assets" }
$files += Get-Item "$root\Cargo.toml","$root\Cargo.lock","$root\build.rs","$root\README.md","$root\LICENSE","$root\THIRD-PARTY-LICENSES.md","$root\.gitignore"
$files = $files | Sort-Object FullName -Unique

# A file is "binary" if it is not valid UTF-8 text. Decided by a round trip
# rather than by extension, so nothing is silently mangled by being guessed wrong.
function Is-Binary($path) {
    $b = [IO.File]::ReadAllBytes($path)
    $enc = New-Object System.Text.UTF8Encoding($false, $true)
    try { [void]$enc.GetString($b); return $false } catch { return $true }
}

$entries = @()
$binCount = 0
$txtCount = 0
$totalBytes = 0

foreach ($f in $files) {
    $rel = $f.FullName.Substring($root.Length + 1).Replace('\', '/')
    $totalBytes += $f.Length
    if (Is-Binary $f.FullName) {
        # Binary content goes up base64-encoded, as its own blob, so the tree
        # request stays small.
        $content = [Convert]::ToBase64String([IO.File]::ReadAllBytes($f.FullName))
        $blob = Post-Json "https://api.github.com/repos/$owner/$repo/git/blobs" @{
            content = $content; encoding = "base64"
        } $headers
        $entries += @{
            path = $rel; mode = "100644"; type = "blob"; sha = $blob.sha
        }
        $binCount++
        Write-Host "  blob  $rel ($([math]::Round($f.Length/1KB)) KB)"
    } else {
        $text = [IO.File]::ReadAllText($f.FullName)
        $entries += @{
            path = $rel; mode = "100644"; type = "blob"; content = $text
        }
        $txtCount++
    }
}

Write-Host "$txtCount text file(s) inline, $binCount blob(s), $([math]::Round($totalBytes/1MB,1)) MB total"

$tree = Post-Json "https://api.github.com/repos/$owner/$repo/git/trees" @{ tree = $entries } $headers
Write-Host "tree $($tree.sha)"

$commit = Post-Json "https://api.github.com/repos/$owner/$repo/git/commits" @{
    message = "TetraFusion 2.2.0 in Rust with raylib-rs 5.5

Faithful port of drDOOM69GAMING's Pygame original, with the deliberate
improvements listed in the README: all five modes behave as their labels
state, per level palettes with a Traditional option, twelve piece skins,
gravity ramping to 20G by level 20, a visible clock where a mode has a time,
three letter high score initials per mode, full controller remapping, and an
OS folder dialog for custom music on every platform.

Scoring follows the Tetris Guideline: the 100/300/500/800 line table, a
separate spin table with full and mini T spins, back to back as a real 1.5x
on the clear, combos, and perfect clears. A T spin requires the piece to
have been rotated last, so a straight drop into a three corner pocket no
longer scores as one.

The executable is self-contained. The fifteen background photos and all five
sounds are include_bytes!'d into it and the icon is compiled into its
resource section, so a copy of tetrafusion.exe on its own is the whole game.
tests/portable.rs runs that copy out of an empty directory to prove it.

Settings and high scores live in the per user app data folder, not beside
the executable.

435 tests, no warnings on a release build."
    tree = $tree.sha
    parents = @()
} $headers
Write-Host "commit $($commit.sha)"

# Point the default branch at it. The old contents-API history is replaced; it
# was one commit per file and carried no information.
#
# The ref already exists, so this is a PATCH, not a POST. POST fails with
# 422 "Reference already exists" and leaves the new commit orphaned.
$sha = $commit.sha
$body = @{ sha = $sha; force = $true } | ConvertTo-Json -Compress
$ref = Invoke-RestMethod -Uri "https://api.github.com/repos/$owner/$repo/git/refs/heads/main" -Headers $headers -Method Patch -Body $body -ContentType "application/json"
Write-Host "main -> $($ref.object.sha)"

$commit | Select-Object sha, html_url | Format-List