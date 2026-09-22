# Builds the Windows installer and keeps this window open to show the result.
$ErrorActionPreference = "Stop"

try {
	Set-Location $PSScriptRoot
	$npmCommand = Get-Command npm -ErrorAction SilentlyContinue
	$npmExecutable = if ($npmCommand) { $npmCommand.Source } else { $null }
	if (-not $npmExecutable -and (Test-Path "${env:ProgramFiles}\nodejs\npm.cmd")) {
		$npmExecutable = "${env:ProgramFiles}\nodejs\npm.cmd"
	}
	if (-not $npmExecutable) {
		throw "npm was not found. Install Node.js LTS from https://nodejs.org/en/download, close this window, then run this script again."
	}
	$npmDirectory = Split-Path -Parent $npmExecutable
	if (($env:Path -split ';') -notcontains $npmDirectory) {
		$env:Path = "$npmDirectory;$env:Path"
	}

	if (-not (Test-Path "node_modules")) {
		Write-Host "Installing locked dependencies..." -ForegroundColor Cyan
		& $npmExecutable ci
		if ($LASTEXITCODE -ne 0) { throw "npm ci exited with code $LASTEXITCODE" }
	}

	Write-Host "Building the Windows installer..." -ForegroundColor Cyan
	& $npmExecutable run build:win
	if ($LASTEXITCODE -ne 0) { throw "npm run build:win exited with code $LASTEXITCODE" }
	$releaseDirectory = Join-Path $PSScriptRoot "release"
	Write-Host "`nBuild completed. Output: $releaseDirectory" -ForegroundColor Green
	Get-ChildItem -LiteralPath $releaseDirectory -File |
		Where-Object { $_.Extension -in ".exe", ".zip" } |
		ForEach-Object { Write-Host " - $($_.Name)" -ForegroundColor Green }
	Write-Host "Portable folder: $releaseDirectory\win-unpacked" -ForegroundColor Green
} catch {
	Write-Host "`nBuild failed: $($_.Exception.Message)" -ForegroundColor Red
} finally {
	Write-Host "`nPress any key to close..." -ForegroundColor Yellow
	$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
}
