# Gera um dataset sintético com as vozes do Windows (SAPI), para validar a
# ferramenta e comparar motores rapidamente. NÃO substitui gravações reais:
# voz sintética é limpa demais e favorece todos os motores. O dataset de
# verdade (vozes do time, sotaques, ruído de escritório) vai em dataset/.
#
# Salvo em UTF-8 COM BOM: o PowerShell 5.1 lê UTF-8 sem BOM como Windows-1252.
#
# Uso: powershell -File tools/benchmark/synth-dataset.ps1 [-Out tools/benchmark/dataset-sintetico]
param([string]$Out = "tools/benchmark/dataset-sintetico")

Add-Type -AssemblyName System.Speech
$fmt = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)

$sets = @{
  "pt-BR" = @{
    Voice = "Microsoft Maria Desktop"
    Lines = @(
      "Bom dia. O relatório de vendas ficou pronto ontem à tarde, e a reunião foi marcada para quinta-feira.",
      "Por favor, envie a proposta revisada até sexta-feira, com os valores atualizados.",
      "A eSales integrou o sistema de pedidos ao ERP do cliente em três semanas.",
      "Precisamos conversar sobre o orçamento do próximo trimestre antes da diretoria.",
      "O atendimento recebeu quarenta e duas solicitações hoje; a maioria já foi resolvida.",
      "Vou chegar um pouco atrasado porque o trânsito na marginal está parado."
    )
  }
  "en" = @{
    Voice = "Microsoft Zira Desktop"
    Lines = @(
      "Please send me the quarterly report before the meeting on Thursday.",
      "The deployment failed because the database migration timed out.",
      "Let's schedule a follow-up call with the customer next week."
    )
  }
}

foreach ($lang in $sets.Keys) {
  $dir = Join-Path $Out $lang
  New-Item -ItemType Directory -Force $dir | Out-Null
  $i = 1
  foreach ($line in $sets[$lang].Lines) {
    $name = "{0:d2}" -f $i
    $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
    $synth.SelectVoice($sets[$lang].Voice)
    $synth.SetOutputToWaveFile((Join-Path $dir "$name.wav"), $fmt)
    $synth.Speak($line)
    $synth.Dispose()
    Set-Content -Path (Join-Path $dir "$name.txt") -Value $line -Encoding utf8
    $i++
  }
}
Write-Output "Dataset sintético em $Out"
