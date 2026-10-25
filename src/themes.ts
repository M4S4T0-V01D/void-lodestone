import type { Theme } from './types';

/** The VOID suite's built-in themes, shared with VOID Workshop. */
export const THEMES: Theme[] = [
  {
    id: 'void',
    name: 'Void',
    mode: 'dark',
    colors: {
      bg: '#0b0a12',
      panel: '#121020',
      panelAlt: '#1a1730',
      border: '#2a2546',
      text: '#e8e4ff',
      textDim: '#9a93c0',
      accent: '#a970ff',
      accent2: '#22e3ff',
      success: '#3ee08f',
      warning: '#ffc857',
      danger: '#ff5c7a',
    },
  },
  {
    id: 'nebula',
    name: 'Nebula',
    mode: 'dark',
    colors: {
      bg: '#0e0814',
      panel: '#170d22',
      panelAlt: '#22142f',
      border: '#3a2350',
      text: '#f5e8ff',
      textDim: '#b394c8',
      accent: '#ff5fd2',
      accent2: '#4df5d1',
      success: '#5ee6a0',
      warning: '#ffd166',
      danger: '#ff6b6b',
    },
  },
  {
    id: 'abyss',
    name: 'Abyss',
    mode: 'dark',
    colors: {
      bg: '#05080f',
      panel: '#0a101c',
      panelAlt: '#111a2b',
      border: '#1e2b45',
      text: '#dce8ff',
      textDim: '#7f93b8',
      accent: '#5b8cff',
      accent2: '#00e0ff',
      success: '#2fd6a0',
      warning: '#f5c451',
      danger: '#ff5577',
    },
  },
  {
    id: 'phosphor',
    name: 'Phosphor',
    mode: 'dark',
    colors: {
      bg: '#060a06',
      panel: '#0b120b',
      panelAlt: '#111c11',
      border: '#1f331f',
      text: '#c9ffcf',
      textDim: '#79a77f',
      accent: '#39ff6a',
      accent2: '#ffb000',
      success: '#39ff6a',
      warning: '#ffb000',
      danger: '#ff4f4f',
    },
  },
  {
    id: 'ember',
    name: 'Ember',
    mode: 'dark',
    colors: {
      bg: '#0f0a0a',
      panel: '#181010',
      panelAlt: '#221616',
      border: '#3a2424',
      text: '#ffece4',
      textDim: '#c49a8c',
      accent: '#ff7a3d',
      accent2: '#c77dff',
      success: '#5ad38b',
      warning: '#ffcc4d',
      danger: '#ff4d6d',
    },
  },
  {
    id: 'daylight',
    name: 'Daylight',
    mode: 'light',
    colors: {
      bg: '#f6f5fb',
      panel: '#ffffff',
      panelAlt: '#eeebf8',
      border: '#d6d1ea',
      text: '#1c1830',
      textDim: '#5d5878',
      accent: '#7a3cff',
      accent2: '#0090b8',
      success: '#16a35f',
      warning: '#b7791f',
      danger: '#d23a5a',
    },
  },
  {
    id: 'high-contrast',
    name: 'High contrast',
    mode: 'dark',
    colors: {
      bg: '#000000',
      panel: '#0a0a0a',
      panelAlt: '#161616',
      border: '#5a5a5a',
      text: '#ffffff',
      textDim: '#d0d0d0',
      accent: '#d48cff',
      accent2: '#00ffff',
      success: '#00ff7f',
      warning: '#ffd400',
      danger: '#ff4d4d',
    },
  },
];

export function findTheme(id: string): Theme {
  return THEMES.find((t) => t.id === id) ?? THEMES[0]!;
}

export function mix(a: string, b: string, t: number): string {
  const pa = parseInt(a.slice(1), 16);
  const pb = parseInt(b.slice(1), 16);
  const ch = (shift: number) => Math.round(((pa >> shift) & 255) * (1 - t) + ((pb >> shift) & 255) * t);
  return `#${[16, 8, 0].map((s) => ch(s).toString(16).padStart(2, '0')).join('')}`;
}

/** The CSS variables a theme sets, including the soft tints and hover colour derived from it. */
export function themeVars(t: Theme): Record<string, string> {
  const c = t.colors;
  return {
    '--bg': c.bg,
    '--panel': c.panel,
    '--panel-alt': c.panelAlt,
    '--border': c.border,
    '--text': c.text,
    '--text-dim': c.textDim,
    '--accent': c.accent,
    '--accent-2': c.accent2,
    '--success': c.success,
    '--warning': c.warning,
    '--danger': c.danger,
    '--accent-soft': mix(c.panel, c.accent, 0.18),
    '--accent-2-soft': mix(c.panel, c.accent2, 0.16),
    '--danger-soft': mix(c.panel, c.danger, 0.16),
    '--warning-soft': mix(c.panel, c.warning, 0.16),
    '--success-soft': mix(c.panel, c.success, 0.16),
    '--hover': mix(c.panel, c.text, 0.06),
  };
}

export function applyTheme(t: Theme): void {
  const root = document.documentElement;
  for (const [k, v] of Object.entries(themeVars(t))) root.style.setProperty(k, v);
  root.style.colorScheme = t.mode;
  root.dataset.mode = t.mode;
}
