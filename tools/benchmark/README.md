# Benchmark de STT (Sprint 0, ADR-003)

Mede, por motor, o **WER** (taxa de erro de palavras, com acentos contando) e a **latência** p50/p95. O resultado define o provedor padrão (`engine.cloudProvider`) e o modelo local.

```sh
# Dataset sintético (vozes do Windows) — só para validar a ferramenta:
powershell -File tools/benchmark/synth-dataset.ps1

# Rodar (chaves da nuvem pelo ambiente: GROQ_API_KEY, OPENAI_API_KEY, DEEPGRAM_API_KEY):
cargo run --release -p anotai-benchmark -- \
  --dataset tools/benchmark/dataset \
  --engines groq,openai,deepgram,local:base,local:small \
  --csv resultados.csv
```

- `--models-dir` aponta para os modelos locais (padrão: `%APPDATA%\AnotAI\models`, onde o app os baixa).
- `--price motor=valor` acrescenta a coluna US$/hora de áudio. Confira o preço atual no site de cada provedor; a ferramenta não traz valores fixos.

## Dataset

Uma pasta por idioma, com pares `nome.wav` (PCM 16 bits, qualquer taxa) + `nome.txt` (o que foi dito, com pontuação e acentos):

```
dataset/
  pt-BR/  01.wav 01.txt  02.wav 02.txt …
  en/     01.wav 01.txt …
```

O dataset de verdade deve ter vozes do time e de usuários-piloto, sotaques variados, microfones de notebook e headset, e ruído de escritório. **Gravações de voz são dado pessoal (LGPD):** colete com consentimento, guarde em armazenamento com acesso controlado e não versione no git (`dataset/` está no `.gitignore`).

## Referência (voz sintética, CPU Intel i7-1365U, 2026-11-05)

| Motor       |   WER | Latência p50 |
| ----------- | ----: | -----------: |
| local:tiny  | 13,6% |        1,3 s |
| local:base  |  8,2% |        2,7 s |
| local:small |  4,5% |        9,6 s |

Voz sintética é limpa demais: esses números só servem para comparar motores entre si.
