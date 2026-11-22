import type { Pattern } from './types';

export const PATTERNS: { id: Pattern; name: string }[] = [
  { id: 'blank', name: '空白' },
  { id: 'ruled', name: '横线' },
  { id: 'grid', name: '方格' },
  { id: 'cross', name: '十字' },
  { id: 'dots', name: '点阵' },
];
