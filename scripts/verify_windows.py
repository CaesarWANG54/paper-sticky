"""Exercise the real Windows / WebView2 build, with an isolated notes directory."""
import ctypes as C
from ctypes import wintypes as W
import json
import os
from pathlib import Path
import subprocess
import time
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
QA = Path(os.environ.get('PAPER_STICKY_QA_DIR', str(ROOT / '.qa')))
DATA = QA / 'data' / 'notes.json'
PIDFILE = QA / 'app.pid'
user = C.windll.user32
user.SetProcessDpiAwarenessContext(C.c_void_p(-4))
user.GetWindowLongPtrW.argtypes = [W.HWND, C.c_int]
user.GetWindowLongPtrW.restype = C.c_ssize_t
user.GetWindowRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
user.GetDpiForWindow.argtypes = [W.HWND]
user.SetForegroundWindow.argtypes = [W.HWND]
checks = []
PUMP = None

def check(name, condition):
    if not condition:
        raise AssertionError(name)
    checks.append(name)
    print('PASS:', name, flush=True)

def wait_for(test, timeout=8):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            value = test()
            if value:
                return value
        except (OSError, ValueError):
            pass
        if PUMP:
            PUMP.wait_for_timeout(80)
        else:
            time.sleep(.08)
    raise AssertionError('Timed out waiting for condition')

def data():
    return json.loads(DATA.read_text(encoding='utf-8'))

def note_window(pid):
    found = []
    @C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
    def visit(hwnd, _):
        owner = W.DWORD()
        user.GetWindowThreadProcessId(hwnd, C.byref(owner))
        r = W.RECT()
        user.GetWindowRect(hwnd, C.byref(r))
        if owner.value == pid and user.IsWindowVisible(hwnd) and r.right-r.left > 260 and r.bottom-r.top > 200:
            found.append((hwnd, (r.left, r.top, r.right, r.bottom)))
        return True
    user.EnumWindows(visit, 0)
    return found[0] if found else None

def drag(start, end):
    user.SetCursorPos(*start)
    user.mouse_event(2, 0, 0, 0, 0)
    time.sleep(.12)
    for n in range(1, 15):
        user.SetCursorPos(round(start[0]+(end[0]-start[0])*n/14), round(start[1]+(end[1]-start[1])*n/14))
        time.sleep(.025)
    user.mouse_event(4, 0, 0, 0, 0)
    time.sleep(.7)

def get_note(browser):
    return next((p for p in browser.contexts[0].pages if 'note.html' in p.url and not p.is_closed()), None)

def label(page):
    return page.evaluate('window.__TAURI_INTERNALS__.metadata.currentWindow.label').replace('note-', '')

with sync_playwright() as pw:
    browser = pw.chromium.connect_over_cdp('http://127.0.0.1:9336')
    PUMP = next(p for p in browser.contexts[0].pages if 'tab.html' in p.url)
    page = get_note(browser)
    page.wait_for_selector('.nb')
    page.evaluate('document.fonts.ready')
    check('Bundled Xiaolai font loaded', page.evaluate('document.fonts.check("23px Xiaolai")'))
    note_id = label(page)
    sample = '今天，慢慢来。\n把想法写下来\n给重要的事留一点时间。'
    page.locator('.nb').fill(sample)
    wait_for(lambda: data()['notes'][0]['content'] == sample)
    check('Chinese text automatically persisted', True)
    page.get_by_role('button', name='纸张样式', exact=True).click()
    for pattern, name in [('blank', '空白'), ('ruled', '横线'), ('grid', '方格'), ('cross', '十字'), ('dots', '点阵')]:
        page.get_by_role('button', name=name, exact=True).click()
        wait_for(lambda: data()['notes'][0]['pattern'] == pattern)
        check(f'Pattern {pattern} selects and saves', page.locator('.note').get_attribute('data-pattern') == pattern)
    page.get_by_role('button', name='鼠尾草绿', exact=True).click()
    wait_for(lambda: data()['notes'][0]['color'] == 'mint')
    check('Preset color saved', True)
    page.locator('input[type=color]').evaluate("el => { el.value='#20352a'; el.dispatchEvent(new Event('input',{bubbles:true})); }")
    wait_for(lambda: data()['notes'][0]['color'] == '#20352a')
    check('Custom color saves with legible light ink', page.locator('.note').evaluate("el => getComputedStyle(el).getPropertyValue('--note-ink')").strip() == '#faf6e9')
    page.get_by_role('button', name='奶油黄', exact=True).click()
    page.get_by_role('button', name='十字', exact=True).click()
    page.wait_for_timeout(350)
    page.screenshot(path=str(QA/'appearance.png'), omit_background=True)
    page.get_by_role('button', name='关闭样式设置', exact=True).click()
    page.screenshot(path=str(QA/'note.png'), omit_background=True)
    pid = int(PIDFILE.read_text())
    hwnd, rect = note_window(pid)
    check('Native window initially always on top', bool(user.GetWindowLongPtrW(hwnd, -20) & 8))
    page.get_by_role('button', name='置顶', exact=True).click()
    wait_for(lambda: not (user.GetWindowLongPtrW(hwnd, -20) & 8))
    check('Pin button removes native topmost flag', True)
    page.get_by_role('button', name='置顶', exact=True).click()
    wait_for(lambda: user.GetWindowLongPtrW(hwnd, -20) & 8)
    check('Pin button restores native topmost flag', True)
    scale = user.GetDpiForWindow(hwnd)/96
    grip = page.locator('.grip').bounding_box()
    start = (round(rect[0]+(grip['x']+grip['width']/2)*scale), round(rect[1]+(grip['y']+grip['height']/2)*scale))
    user.SetForegroundWindow(hwnd)
    drag(start, (start[0]-160, start[1]+70))
    moved = note_window(pid)[1]
    check('Native mouse drag moves paper', abs(moved[0]-rect[0]) > 80)
    wait_for(lambda: abs(data()['notes'][0]['window']['x']-(moved[0]/scale+16)) < 3)
    check('Dragged position automatically saved', True)
    resize = page.locator('.rs').bounding_box()
    start = (round(moved[0]+(resize['x']+10)*scale), round(moved[1]+(resize['y']+10)*scale))
    drag(start, (start[0]+60, start[1]+50))
    resized = note_window(pid)[1]
    check('Native corner drag resizes paper', resized[2]-resized[0] > moved[2]-moved[0]+30)
    area=W.RECT(); user.SystemParametersInfoW(0x30, 0, C.byref(area), 0)
    grip=page.locator('.grip').bounding_box()
    start=(round(resized[0]+(grip['x']+grip['width']/2)*scale), round(resized[1]+(grip['y']+grip['height']/2)*scale))
    dx=area.right-(resized[2]-round(16*scale))-4
    drag(start, (start[0]+dx, start[1]))
    wait_for(lambda: not data()['notes'][0]['open'])
    check('Dragging to primary right edge docks paper', True)
    tab = next(p for p in browser.contexts[0].pages if 'tab.html' in p.url)
    tab.locator('.dot').first.click()
    page = wait_for(lambda: get_note(browser))
    page.wait_for_selector('.nb')
    check('Dock reopens the saved note', page.locator('.nb').inner_text() == sample)
    check('Dock reopens the chosen pattern', page.locator('.note').get_attribute('data-pattern') == 'cross')
    page.locator('.nb').fill(sample+'\n这句立刻收纳也会保存。')
    page.get_by_role('button',name='收纳便签',exact=True).click()
    wait_for(lambda: not data()['notes'][0]['open'])
    check('Immediate dock waits for latest text to save', '立刻收纳' in data()['notes'][0]['content'])
    tab.locator('.dot').first.click()
    page=wait_for(lambda: get_note(browser)); page.wait_for_selector('.nb')
    page.locator('.new-note').click()
    wait_for(lambda: len(data()['notes']) == 2)
    second=wait_for(lambda: next((p for p in browser.contexts[0].pages if 'note.html' in p.url and not p.is_closed() and label(p)!=note_id),None))
    second.wait_for_selector('.nb')
    second.locator('.nb').fill('另一张便签，独立保存。')
    second.locator('.appearance').click(); second.get_by_role('button',name='雾蓝',exact=True).click(); second.get_by_role('button',name='横线',exact=True).click()
    wait_for(lambda: data()['notes'][1]['pattern']=='ruled')
    check('Multiple notes keep separate content and appearance', data()['notes'][0]['pattern']=='cross' and data()['notes'][1]['color']=='sky')
    saved=data()
    (QA/'expected-restart.json').write_text(json.dumps(saved,ensure_ascii=False,indent=2),encoding='utf-8')
    check('Automatic backup exists', DATA.with_suffix('.json.bak').exists())

(QA/'windows-checks.json').write_text(json.dumps(checks,ensure_ascii=False,indent=2),encoding='utf-8')
print('Completed',len(checks),'Windows checks',flush=True)
