$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = New-Object System.Text.UTF8Encoding($false)
try {
    $request = [Console]::In.ReadToEnd() | ConvertFrom-Json
    $target = [IO.Path]::GetFullPath($request.executable)
    $icon = [IO.Path]::GetFullPath($request.icon)
    if (!(Test-Path -LiteralPath $target -PathType Leaf) -or !(Test-Path -LiteralPath $icon -PathType Leaf)) { throw 'Missing application icon' }
    $shell = New-Object -ComObject WScript.Shell
    $desktop = [Environment]::GetFolderPath('Desktop')
    $roots = @($desktop, [Environment]::GetFolderPath('Programs'), (Join-Path $env:APPDATA 'Microsoft/Internet Explorer/Quick Launch/User Pinned/TaskBar'))
    foreach ($directory in $roots) {
        if (!(Test-Path -LiteralPath $directory -PathType Container)) { continue }
        foreach ($file in (Get-ChildItem -LiteralPath $directory -Filter '*.lnk' -File -Recurse -ErrorAction SilentlyContinue)) {
            $shortcut = $shell.CreateShortcut($file.FullName)
            if ($shortcut.TargetPath -and [IO.Path]::GetFullPath($shortcut.TargetPath) -ieq $target) {
                $shortcut.IconLocation = $icon + ',0'
                $shortcut.Description = 'Veirune'
                $shortcut.Save()
            }
        }
    }
    $desktopLink = Join-Path $desktop 'Veirune.lnk'
    if ($request.create -and !(Test-Path -LiteralPath $desktopLink)) {
        $shortcut = $shell.CreateShortcut($desktopLink)
        $shortcut.TargetPath = $target
        $shortcut.WorkingDirectory = [IO.Path]::GetDirectoryName($target)
        $shortcut.IconLocation = $icon + ',0'
        $shortcut.Description = 'Veirune'
        $shortcut.Save()
    }
    Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class VeiruneShell { [DllImport("shell32.dll")] public static extern void SHChangeNotify(uint e, uint f, IntPtr a, IntPtr b); }'
    [VeiruneShell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
    exit 0
} catch { exit 1 }
