import { COLORS, colorHex, inkFor } from '../shared/colors';
import { $$ } from '../shared/dom';
import { ICON } from '../shared/icons';
import { PATTERNS } from '../shared/paper';
import type { Color, Pattern, Shape } from '../shared/types';

export interface ShellState {
  color: Color;
  shape?: Shape;
  pattern?: Pattern;
  pinned: boolean;
}
const paletteIcon =
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M12 3a9 9 0 1 0 0 18h1.2a2 2 0 0 0 1.5-3.3c-.7-.8-.2-2.1.8-2.1H17a4 4 0 0 0 4-4C21 6.9 17 3 12 3Z"/><circle cx="7.5" cy="10" r=".9"/><circle cx="11" cy="7" r=".9"/><circle cx="15.5" cy="8.3" r=".9"/></svg>';
const dockIcon =
  '<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M17 4v12M3 10h10m-4-4 4 4-4 4"/></svg>';

export function createNoteShell(state: ShellState): HTMLElement {
  const el = document.createElement('div');
  el.className = 'note shape-square';
  el.innerHTML = `
    <div class="paper" aria-hidden="true"></div>
    <div class="inner">
      <div class="nh">
        <button class="ic appearance" title="纸张样式" aria-label="纸张样式" aria-expanded="false">${paletteIcon}</button>
        <button class="ic new-note" title="新便签 · Ctrl+N" aria-label="新便签">${ICON.plus}</button>
        <div class="grip" title="拖动便签，拖到屏幕右边缘收纳"></div>
        <button class="ic pin" aria-label="置顶">${ICON.pin}</button>
        <button class="ic close" title="收纳到右侧 · Ctrl+W" aria-label="收纳便签">${dockIcon}</button>
      </div>
      <div class="nb" aria-label="便签内容"></div>
    </div>
    <div class="curl" aria-hidden="true"></div>
    <div class="rs" title="调整大小"></div>
    <section class="appearance-panel" aria-label="纸张样式设置" hidden>
      <div class="panel-title"><span>纸张样式</span><button class="ic dismiss" aria-label="关闭样式设置">${ICON.close}</button></div>
      <div class="panel-label">背景颜色</div>
      <div class="swatches">${COLORS.map((c) => `<button class="swo" data-color="${c.id}" style="background:${c.hex}" title="${c.name}" aria-label="${c.name}" aria-pressed="false"></button>`).join('')}
        <label class="custom-color" title="自定义颜色"><input type="color" aria-label="自定义背景颜色" value="${colorHex(state.color)}"/><span>＋</span></label>
      </div>
      <div class="panel-label">纸面花样</div>
      <div class="patterns">${PATTERNS.map((p) => `<button class="pattern-option" data-pattern="${p.id}" aria-label="${p.name}" aria-pressed="false"><span class="pattern-sample pattern-${p.id}"></span><span>${p.name}</span></button>`).join('')}</div>
      <div class="panel-footer"><span>小赖手写体</span><button class="delete-note" title="删除便签">${ICON.trash}<span>删除</span></button></div>
    </section>
    <button class="save-error" hidden aria-live="polite">未保存，点击重试</button>`;
  applyColor(el, state.color);
  applyPattern(el, state.pattern ?? 'blank');
  applyPin(el, state.pinned);
  return el;
}

export function applyColor(el: HTMLElement, color: Color): void {
  const ink = inkFor(color);
  el.style.setProperty('--paper', colorHex(color));
  el.style.setProperty('--note-ink', ink);
  el.style.setProperty(
    '--rule',
    ink === '#faf6e9' ? 'rgba(255,255,245,.19)' : 'rgba(70,82,65,.17)',
  );
  for (const s of $$('.swo', el)) {
    const selected = s.dataset.color === color;
    s.classList.toggle('on', selected);
    s.setAttribute('aria-pressed', String(selected));
  }
  const custom = el.querySelector<HTMLInputElement>('input[type="color"]');
  if (custom) custom.value = colorHex(color);
}

export function applyPattern(el: HTMLElement, pattern: Pattern): void {
  el.dataset.pattern = pattern;
  for (const p of $$('.pattern-option', el)) {
    const selected = p.dataset.pattern === pattern;
    p.classList.toggle('on', selected);
    p.setAttribute('aria-pressed', String(selected));
  }
}

export function applyShape(_el: HTMLElement, _shape: Shape): void {}

export function applyPin(el: HTMLElement, pinned: boolean): void {
  const pin = el.querySelector<HTMLElement>('.pin');
  if (!pin) return;
  pin.classList.toggle('on', pinned);
  pin.setAttribute('aria-pressed', String(pinned));
  pin.title = pinned ? '已置顶 · 点击取消' : '置顶显示';
}
