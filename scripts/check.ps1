$ErrorActionPreference = "Stop"

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo test --workspace
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (Get-Command npm -ErrorAction SilentlyContinue) {
    Push-Location apps/watchmind-web
    npm run build
    if ($LASTEXITCODE -ne 0) {
        Pop-Location
        exit $LASTEXITCODE
    }
    npm run budget
    if ($LASTEXITCODE -ne 0) {
        Pop-Location
        exit $LASTEXITCODE
    }
    Pop-Location
}
