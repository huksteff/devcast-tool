@"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -Command "& ([scriptblock]::Create([IO.File]::ReadAllText('%~dp0mock.ps1')))"
