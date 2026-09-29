param([switch]$RebuildWorker)

$ErrorActionPreference = 'Stop'
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$bundle = Join-Path $project 'desktop/src-tauri/resources/clap'
$model = Join-Path $bundle 'model'
$worker = Join-Path $bundle 'worker-lite/fastcloud-clap'
$venv = Join-Path $env:LOCALAPPDATA 'fastcloud/clap-lite-venv'
$python = Join-Path $venv 'Scripts/python.exe'
$hf = Join-Path $venv 'Scripts/hf.exe'
$revision = 'e9fd5ac1dbf3280936a7fc3ec8a020453ff184db'
$privateConfig = Join-Path $project 'desktop/src-tauri/tauri.private.conf.json'
$licenseDir = Join-Path $bundle 'licenses'
New-Item -ItemType Directory -Force -Path $licenseDir | Out-Null
Copy-Item -LiteralPath (Join-Path $project 'assets/licenses/NOTICE-CLAP.txt') -Destination $licenseDir -Force
Copy-Item -LiteralPath (Join-Path $project 'assets/licenses/LICENSE-APACHE-2.0.txt') -Destination $licenseDir -Force

if (-not (Test-Path -LiteralPath $privateConfig)) {
    @'
{
  "bundle": {
    "resources": [
      "resources/clap/worker-lite/fastcloud-clap/**/*",
      "resources/clap/model/*.json",
      "resources/clap/model/*.txt",
      "resources/clap/model/onnx/audio_model_quantized.onnx",
      "resources/clap/model/onnx/text_model_quantized.onnx",
      "resources/clap/licenses/*.txt"
    ]
  }
}
'@ | Set-Content -LiteralPath $privateConfig -Encoding utf8
}

if (-not (Test-Path -LiteralPath $python)) {
    uv venv --python 3.12 $venv
    if ($LASTEXITCODE -ne 0) { throw 'Could not create the CLAP build environment.' }
}
uv pip install --python $python 'onnxruntime==1.30.0' 'tokenizers==0.22.2' 'pyinstaller==6.22.3' 'huggingface_hub[cli]'
if ($LASTEXITCODE -ne 0) { throw 'Could not install CLAP build dependencies.' }

& $hf download Xenova/larger_clap_music_and_speech `
    config.json preprocessor_config.json tokenizer.json tokenizer_config.json `
    special_tokens_map.json vocab.json merges.txt `
    onnx/audio_model_quantized.onnx onnx/text_model_quantized.onnx `
    --revision $revision --local-dir $model
if ($LASTEXITCODE -ne 0) { throw 'Could not download the pinned CLAP weights.' }

$expected = @{
    'audio_model_quantized.onnx' = '021DD4FB962B4ED20CC3A6730B09E0CCF9F9C49931047032118A3B64828513A6'
    'text_model_quantized.onnx' = '8F9F29C5F6ADEE917553D4B3A70729C731C0D18B88EFCA0AE67C1A1FC278F3B6'
}
foreach ($name in $expected.Keys) {
    $actual = (Get-FileHash -LiteralPath (Join-Path $model "onnx/$name") -Algorithm SHA256).Hash
    if ($actual -ne $expected[$name]) { throw "CLAP model hash mismatch: $name" }
}

if ($RebuildWorker -and (Test-Path -LiteralPath $worker)) {
    $resolvedProject = [IO.Path]::GetFullPath($project).TrimEnd('\') + '\'
    $resolvedWorker = [IO.Path]::GetFullPath($worker).TrimEnd('\')
    if (-not $resolvedWorker.StartsWith($resolvedProject, [StringComparison]::OrdinalIgnoreCase) -or
        $resolvedWorker -ne [IO.Path]::GetFullPath((Join-Path $project 'desktop/src-tauri/resources/clap/worker-lite/fastcloud-clap')).TrimEnd('\')) {
        throw 'Refusing to remove a worker outside the intended workspace folder.'
    }
    Remove-Item -LiteralPath $resolvedWorker -Recurse -Force
}

if (-not (Test-Path -LiteralPath (Join-Path $worker 'fastcloud-clap.exe'))) {
    $work = Join-Path $env:LOCALAPPDATA 'fastcloud/clap-lite-pyi-build'
    & $python -m PyInstaller --onedir --name fastcloud-clap `
        --distpath (Join-Path $bundle 'worker-lite') --workpath $work --specpath $work `
        (Join-Path $project 'desktop/clap_worker.py')
    if ($LASTEXITCODE -ne 0) { throw 'Could not freeze the CLAP worker.' }
}

Write-Host "Private CLAP resources ready in $bundle"
Write-Host 'Build locally with: cd desktop; npm run tauri -- build --config src-tauri/tauri.private.conf.json --bundles nsis'
