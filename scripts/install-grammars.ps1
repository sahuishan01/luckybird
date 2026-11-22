<#
.SYNOPSIS
  Build and install tree-sitter grammars + highlight queries for luckybird.

.DESCRIPTION
  For each requested language this script:
    1. Clones https://github.com/tree-sitter/tree-sitter-<lang>
    2. Runs `tree-sitter build` to produce a shared library
    3. Copies <lang>.dll        to %LOCALAPPDATA%\luckybird\grammars\
    4. Copies highlights.scm    to %APPDATA%\luckybird\queries\<lang>\

  Prerequisites:
    - git
    - tree-sitter CLI            (install: `cargo install tree-sitter-cli`)
    - A C compiler on PATH       (MSVC `cl.exe` from Visual Studio Build Tools,
                                  or clang/gcc; tree-sitter picks whichever it
                                  finds)

.PARAMETER Languages
  Languages to install. Defaults to a small starter set. Each name must match
  the `[<lang>]` section in crates/lb-syntax/grammars.toml.

.EXAMPLE
  .\scripts\install-grammars.ps1
  # Installs the default set: rust, python, javascript, json, toml, html, css

.EXAMPLE
  .\scripts\install-grammars.ps1 -Languages rust,go,bash

.EXAMPLE
  .\scripts\install-grammars.ps1 -Languages all
  # Tries to install every language listed in grammars.toml.
#>
param(
    [string[]] $Languages = @('rust','python','javascript','json','toml','html','css')
)

$ErrorActionPreference = 'Stop'

# ── Sanity checks ────────────────────────────────────────────────────────────
foreach ($cmd in 'git','tree-sitter') {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Error "$cmd not found on PATH. See script header for prerequisites."
    }
}

# Grammar repo overrides for languages whose repo isn't `tree-sitter/tree-sitter-<lang>`.
# Format: name = "owner/repo[#subdir]"
$repoOverrides = @{
    'cpp'    = 'tree-sitter/tree-sitter-cpp'
    'csharp' = 'tree-sitter/tree-sitter-c-sharp'
    'tsx'    = 'tree-sitter/tree-sitter-typescript#tsx'
    'typescript' = 'tree-sitter/tree-sitter-typescript#typescript'
    'dockerfile' = 'camdencheek/tree-sitter-dockerfile'
    'markdown'   = 'MDeiml/tree-sitter-markdown#tree-sitter-markdown'
    'elixir'  = 'elixir-lang/tree-sitter-elixir'
    'erlang'  = 'WhatsApp/tree-sitter-erlang'
    'haskell' = 'tree-sitter/tree-sitter-haskell'
    'kotlin'  = 'fwcd/tree-sitter-kotlin'
    'ocaml'   = 'tree-sitter/tree-sitter-ocaml#grammars/ocaml'
    'scala'   = 'tree-sitter/tree-sitter-scala'
    'sql'     = 'derekstride/tree-sitter-sql'
    'swift'   = 'alex-pinkus/tree-sitter-swift'
    'zig'     = 'maxxnino/tree-sitter-zig'
    'fish'    = 'ram02z/tree-sitter-fish'
}

# Resolve "all" to the full list from grammars.toml.
if ($Languages.Count -eq 1 -and $Languages[0] -eq 'all') {
    $tomlPath = Join-Path $PSScriptRoot '..\crates\lb-syntax\grammars.toml'
    $Languages = (Get-Content $tomlPath) `
        | Where-Object { $_ -match '^\[([a-z0-9_]+)\]\s*$' } `
        | ForEach-Object { $matches[1] }
}

$grammarsDir = Join-Path $env:LOCALAPPDATA 'luckybird\grammars'
$queriesDir  = Join-Path $env:APPDATA      'luckybird\queries'
$workDir     = Join-Path $env:TEMP         'luckybird-grammar-build'

New-Item -ItemType Directory -Force $grammarsDir,$queriesDir,$workDir | Out-Null

Write-Host "→ grammars  : $grammarsDir"
Write-Host "→ queries   : $queriesDir"
Write-Host "→ work dir  : $workDir"
Write-Host ""

# ── Per-language install ─────────────────────────────────────────────────────
$failed = @()

foreach ($lang in $Languages) {
    Write-Host "[$lang] ─────────────────────────────────────────"

    $repoSpec = if ($repoOverrides.ContainsKey($lang)) { $repoOverrides[$lang] } `
                else { "tree-sitter/tree-sitter-$lang" }

    $subdir = $null
    if ($repoSpec -match '^(.+)#(.+)$') {
        $repoSpec = $matches[1]; $subdir = $matches[2]
    }
    $repoUrl = "https://github.com/$repoSpec.git"
    $cloneTo = Join-Path $workDir ($repoSpec -replace '/','_')

    if (-not (Test-Path $cloneTo)) {
        Write-Host "  cloning $repoUrl"
        git clone --depth 1 $repoUrl $cloneTo 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "  clone failed"
            $failed += $lang
            continue
        }
    } else {
        Write-Host "  using cached clone at $cloneTo"
    }

    $buildDir = if ($subdir) { Join-Path $cloneTo $subdir } else { $cloneTo }
    $outLib   = Join-Path $grammarsDir "$lang.dll"

    Push-Location $buildDir
    try {
        Write-Host "  building $outLib"
        tree-sitter build --output $outLib 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path $outLib)) {
            Write-Warning "  build failed"
            $failed += $lang
            continue
        }

        # Highlight query — try queries/highlights.scm in the build dir.
        $srcQuery = Join-Path $buildDir 'queries\highlights.scm'
        if (Test-Path $srcQuery) {
            $destQueryDir = Join-Path $queriesDir $lang
            New-Item -ItemType Directory -Force $destQueryDir | Out-Null
            Copy-Item $srcQuery (Join-Path $destQueryDir 'highlights.scm') -Force
            Write-Host "  installed highlights.scm"
        } else {
            Write-Warning "  no queries\highlights.scm in repo (highlight will be limited)"
        }
    } finally {
        Pop-Location
    }
}

Write-Host ""
if ($failed.Count -eq 0) {
    Write-Host "All grammars installed successfully." -ForegroundColor Green
} else {
    Write-Warning "Some languages failed: $($failed -join ', ')"
    Write-Host "Re-run with just one (e.g. -Languages rust) to see full error output."
}
