/** Paleta da identidade AnotAI. Fonte única: espelhada em tokens.css. */
export const brand = {
  violet: "#6D4AFF",
  coral: "#FF5A5F",
  cyan: "#18C8E0",
  night: "#0E0B1F",
} as const;

/** Cor por estado do app (overlay e ícone da bandeja). */
export const stateColor = {
  idle: brand.violet,
  listening: brand.coral,
  translation: brand.cyan,
} as const;

export const fonts = {
  display: '"Sora", system-ui, sans-serif',
  sans: '"Inter", system-ui, sans-serif',
  mono: '"JetBrains Mono", ui-monospace, monospace',
} as const;

export type BrandColor = keyof typeof brand;
