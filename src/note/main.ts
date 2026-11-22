import '../shared/theme.css';
import './note.css';
import './paper.css';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { installPageDefaults, reportError } from '../shared/dom';
import { ipc, onNotesChanged } from '../shared/ipc';
import { parse, serialize } from '../shared/model';
import type { Color, Pattern } from '../shared/types';
import { createBar } from './bar';
import { createEditor } from './editor';
import { applyColor, applyPin, applyPattern, createNoteShell } from './shell';

installPageDefaults();
const win = getCurrentWindow();
const id = win.label.replace(/^note-/, '');

async function main(): Promise<void> {
  const note = await ipc.getNote(id);
  if (!note) return;
  const el = createNoteShell(note);
  document.body.appendChild(el);
  const body = el.querySelector<HTMLElement>('.nb')!;
  const panel = el.querySelector<HTMLElement>('.appearance-panel')!;
  const appearance = el.querySelector<HTMLButtonElement>('.appearance')!;
  const error = el.querySelector<HTMLButtonElement>('.save-error')!;
  let color = note.color;
  let pattern = note.pattern;
  let pinned = note.pinned;
  let closing = false;
  let queue = Promise.resolve();
  let saveRevision = 0;
  let pending: { content?: string; color?: Color; pattern?: Pattern } = {};

  function update(patch: typeof pending): Promise<void> {
    pending = { ...pending, ...patch };
    const revision = ++saveRevision;
    const snapshot = { ...pending };
    queue = queue
      .catch(() => {})
      .then(async () => {
        try {
          await ipc.updateNote(id, snapshot);
          if (revision === saveRevision) {
            pending = {};
            error.hidden = true;
          }
        } catch (err) {
          error.hidden = false;
          throw err;
        }
      });
    queue.catch((err) => console.error('save:', err));
    return queue;
  }
  const editor = createEditor(body, parse(note.content), (blocks) => {
    void update({ content: serialize(blocks) });
  });
  createBar(el.querySelector<HTMLElement>('.inner')!, body, editor);
  error.addEventListener('click', () => void update(pending));
  function togglePanel(open: boolean): void {
    panel.hidden = !open;
    appearance.setAttribute('aria-expanded', String(open));
    el.classList.toggle('settings-open', open);
  }
  appearance.addEventListener('click', () => togglePanel(panel.hidden));
  el.querySelector('.dismiss')!.addEventListener('click', () => togglePanel(false));
  document.addEventListener('pointerdown', (ev) => {
    if (!(ev.target as HTMLElement).closest('.appearance-panel, .appearance')) togglePanel(false);
  });
  el.querySelector('.swatches')!.addEventListener('click', (ev) => {
    const button = (ev.target as HTMLElement).closest<HTMLElement>('[data-color]');
    if (!button) return;
    color = button.dataset.color as Color;
    applyColor(el, color);
    void update({ color });
  });
  el.querySelector<HTMLInputElement>('input[type="color"]')!.addEventListener('input', (ev) => {
    color = (ev.target as HTMLInputElement).value as Color;
    applyColor(el, color);
    void update({ color });
  });
  el.querySelector('.patterns')!.addEventListener('click', (ev) => {
    const button = (ev.target as HTMLElement).closest<HTMLElement>('[data-pattern]');
    if (!button) return;
    pattern = button.dataset.pattern as Pattern;
    applyPattern(el, pattern);
    void update({ pattern });
  });
  const create = async () => {
    await queue;
    await ipc.createNote();
  };
  el.querySelector('.new-note')!.addEventListener('click', () => void create().catch(report));
  el.querySelector('.pin')!.addEventListener('click', async () => {
    const wanted = !pinned;
    try {
      await ipc.setNotePinned(id, wanted);
      pinned = wanted;
      applyPin(el, pinned);
    } catch (err) {
      report(err);
    }
  });
  async function close(): Promise<void> {
    if (closing) return;
    closing = true;
    try {
      await update({ content: serialize(editor.getBlocks()) });
      document.body.classList.add('closing');
      await new Promise((resolve) => setTimeout(resolve, 160));
      await ipc.closeNote(id);
    } catch (err) {
      closing = false;
      report(err);
    }
  }
  el.querySelector('.close')!.addEventListener('click', () => void close());
  void win.onCloseRequested((ev) => {
    ev.preventDefault();
    void close();
  });
  let armTimer: ReturnType<typeof setTimeout> | undefined;
  const del = el.querySelector<HTMLButtonElement>('.delete-note')!;
  del.addEventListener('click', () => {
    if (del.classList.contains('armed')) {
      clearTimeout(armTimer);
      void queue.then(() => ipc.deleteNote(id)).catch(report);
    } else {
      del.classList.add('armed');
      del.querySelector('span')!.textContent = '确认删除';
      armTimer = setTimeout(() => {
        del.classList.remove('armed');
        del.querySelector('span')!.textContent = '删除';
      }, 3000);
    }
  });
  el.querySelector('.grip')!.addEventListener('mousedown', (ev) => {
    if ((ev as MouseEvent).button !== 0) return;
    togglePanel(false);
    void win
      .startDragging()
      .then(() => queue)
      .then(() => ipc.finishDrag(id))
      .catch(report);
  });
  el.querySelector('.rs')!.addEventListener('pointerdown', (ev) => {
    if ((ev as PointerEvent).button !== 0) return;
    ev.preventDefault();
    void win.startResizeDragging('SouthEast').catch(report);
  });
  document.addEventListener('keydown', (ev) => {
    if (ev.isComposing) return;
    if (ev.key === 'Escape') togglePanel(false);
    if (!ev.ctrlKey) return;
    if (ev.key.toLowerCase() === 'w') {
      ev.preventDefault();
      void close();
    }
    if (ev.key.toLowerCase() === 'n') {
      ev.preventDefault();
      void create().catch(report);
    }
    if (ev.key.toLowerCase() === 's') {
      ev.preventDefault();
      void update({ content: serialize(editor.getBlocks()) });
    }
  });
  void listen('flush-notes', () => void update({ content: serialize(editor.getBlocks()) }));
  void onNotesChanged((data) => {
    const mine = data.notes.find((n) => n.id === id);
    if (!mine) return;
    if (mine.color !== color) {
      color = mine.color;
      applyColor(el, color);
    }
    if (mine.pattern !== pattern) {
      pattern = mine.pattern;
      applyPattern(el, pattern);
    }
    if (mine.pinned !== pinned) {
      pinned = mine.pinned;
      applyPin(el, pinned);
    }
  });
  editor.focusEnd();
  function report(err: unknown): void {
    error.hidden = false;
    reportError(String(err));
  }
}
main().catch((err) => reportError(`无法打开便签：${String(err)}`));
