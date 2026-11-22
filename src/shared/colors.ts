import type { Color } from './types';

export const COLORS: { id: Color; hex: string; name: string }[] = [
  { id: 'butter', hex: '#F9E9A1', name: '奶油黄' },
  { id: 'ivory', hex: '#F4F0E4', name: '米白' },
  { id: 'peach', hex: '#F6D6BD', name: '杏桃' },
  { id: 'mint', hex: '#D5E6CD', name: '鼠尾草绿' },
  { id: 'sky', hex: '#D4E5EE', name: '雾蓝' },
  { id: 'lilac', hex: '#E3DCEF', name: '浅紫' },
  { id: 'rose', hex: '#F0D4D9', name: '淡粉' },
];

export function colorHex(id: string): string {
  if (/^#[0-9a-f]{6}$/i.test(id)) return id;
  return (COLORS.find((c) => c.id === id) ?? COLORS[0]).hex;
}

export function inkFor(color: string): string {
  const hex = colorHex(color).slice(1);
  const rgb = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  return rgb[0] * 0.299 + rgb[1] * 0.587 + rgb[2] * 0.114 < 0.48 ? '#faf6e9' : '#393a32';
}
