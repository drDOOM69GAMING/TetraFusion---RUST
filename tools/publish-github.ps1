$ErrorActionPreference = "Stop"

# Publishes the crate to GitHub as ONE commit, parented to the current main.
#
# Two things this gets right that a contents-API upload does not:
#
#   - One tree, one commit, not one commit per file. Uploading through
#     /contents/ produces a hundred commits of "Add src/board.rs" and a history
#     that says nothing.
#   - History is extended, not replaced. An earlier version of this script
#     created a parentless root commit and force-pushed over main. That is
#     destructive and there was no reason for it: this repo is public and one
#     real commit already exists, so new work goes on top of it and `git log`
#     still means something. Only the tag and release move forward.
#
# Deletions are explicit. Building a tree with base_tree inherits every path the
# entries do not mention, so a file removed locally is still on the branch
# unless it is listed with a null sha. That is how examples/ went away.

$token = $env:GH_TOKEN
$owner = "drDOOM69GAMING"
$repo = "TetraFusion---RUST"
$root = "C:\Users\amduser\Documents\Default Project\tetrafusion-rs"
$api = "https://api.github.com/repos/$owner/$repo"

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

# Build-needed files only. No target/, no captures, no local helper scripts:
# the point of the tree is that it is what someone needs to build the game.
$files = Get-ChildItem "$root\src","$root\tests","$root\tools","$root\assets" -Recurse -File |
    Where-Object { $_.Extension -ne ".md" -or $_.DirectoryName -notmatch "assets" } |
    Where-Object { $_.Name -notin @("keys.ps1", "shot.ps1", "vk.ps1") }
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
        $blob = Post-Json "$api/git/blobs" @{ content = $content; encoding = "base64" } $headers
        $entries += @{ path = $rel; mode = "100644"; type = "blob"; sha = $blob.sha }
        $binCount++
    } else {
        $entries += @{
            path = $rel; mode = "100644"; type = "blob"
            content = [IO.File]::ReadAllText($f.FullName)
        }
        $txtCount++
    }
}
Write-Host "$txtCount text file(s) inline, $binCount blob(s), $([math]::Round($totalBytes/1MB,1)) MB total"

# Parent on whatever main is now, so this is an addition to history.
# Parent on whatever main is now, so this is an addition to history.
$ref = Invoke-RestMethod -Uri "$api/git/refs/heads/main" -Headers $headers
$baseCommit = Invoke-RestMethod -Uri "$api/git/commits/$($ref.object.sha)" -Headers $headers
Write-Host "parent $($baseCommit.sha)"

# No base_tree. The entry list is the complete set of files this repo should
# contain, and a tree built from it replaces the tree wholesale, so a path that
# is no longer listed is simply absent - which is how examples/ went away without
# a deletion entry.
#
# An earlier attempt passed base_tree and a deletion entry with a null sha.
# ConvertTo-Json drops null keys entirely, so that entry went over the wire as a
# blob with neither sha nor content, and the API rejected the whole tree with a
# 422 GitRPC::BadObjectState that says nothing about the real cause. Not using
# base_tree is both simpler and one request shorter.
$tree = Post-Json "$api/git/trees" @{ tree = $entries } $headers
Write-Host "tree $($tree.sha)"

$message = @"
TetraFusion 2.2.1: music that keeps up with gravity, visible per-level
backgrounds, and a celebration for a new high score

Three changes, and two of them start from a report that something did not
work.

The backgrounds were already changing per level and already drawn across the
full window width. Measuring them showed why nobody noticed. A flat 39% black
overlay sat over the whole screen, which left every one of the fifteen bundled
photos between 14% and 21% grey. A photo that starts nearly black, dimmed into
being invisible, replaced by a different photo that is also nearly black, is not
a change you can see. The dim is now two values: unchanged behind the well,
where blocks and the ghost piece need the contrast, and much lighter behind the
side panel, where there is nothing to read and where the high score, score and
level live. The same photos now land between 20% and 30% grey there. Both bounds
are pinned by tests that measure the embedded photos rather than the constants.

The music tempo ramp is retied to gravity rather than to the level number. The
old ramp stepped evenly, which was wrong without being obviously wrong, because
the game does not speed up evenly: falling speed doubles from level 1 to level 2
and then flattens towards its floor. The ramp is now a power curve of the same
gravity value, so it has the same shape, capped above anything gravity reaches
inside its own range so the cap never binds while the game is still climbing.

A new high score now throws tetromino pieces across the whole screen for three
seconds. Each shard is a real piece from the shape tables rather than a square,
thrown from the middle in every direction, spinning and falling and fading in
and out. It covers the side panel as well as the well, because a record happened
to the run rather than to a piece, and it draws behind the initials prompt so
your three letters stay readable.

495 tests, no warnings on a release build.
"@

$commit = Post-Json "$api/git/commits" @{
    message = $message
    tree = $tree.sha
    parents = @($baseCommit.sha)
} $headers
Write-Host "commit $($commit.sha)"

# A fast-forward, not a force: if the branch has moved since the parent was
# read, this fails rather than silently discarding whatever landed in between.
$body = @{ sha = $commit.sha; force = $false } | ConvertTo-Json -Compress
$updated = Invoke-RestMethod -Uri "$api/git/refs/heads/main" -Headers $headers -Method Patch -Body $body -ContentType "application/json"
Write-Host "main -> $($updated.object.sha)"

$commit | Select-Object sha, html_url | Format-List