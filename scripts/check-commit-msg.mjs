// Valida Conventional Commits (chamado pelo lefthook no commit-msg).
import { readFileSync } from "node:fs";

const file = process.argv[2];
if (!file) {
  console.error("uso: node scripts/check-commit-msg.mjs <arquivo>");
  process.exit(2);
}
const msg = readFileSync(file, "utf8");
const pattern = /^(feat|fix|chore|docs|refactor|test|perf|build|ci|style|revert)(\(.+\))?!?: .+/;
if (!pattern.test(msg)) {
  console.error(
    "Use Conventional Commits: tipo(escopo): descrição — ex.: feat(hotkey): detectar toque duplo",
  );
  process.exit(1);
}
