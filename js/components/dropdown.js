/**
 * Custom dropdowns. The native <select> stays in the DOM and keeps being the source of truth
 * (value, change events, label for=), the button and the popover only mirror it.
 */

const nativeValue = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value');
const nativeIndex = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'selectedIndex');

let open = null;
let uid = 0;

/** "IQ4_K (4.50 BPW)" -> ["IQ4_K", "4.50 bpw"] */
export function splitLabel(text) {
    const m = text.match(/^(.*?)\s*\(([\d.]+) BPW\)$/);
    return m ? [m[1], `${m[2]} bpw`] : [text, ''];
}

function optionParts(opt) {
    if (!opt) return ['', ''];
    if (opt.dataset.meta !== undefined) return [opt.textContent, opt.dataset.meta];
    return splitLabel(opt.textContent);
}

function place(panel, btn, align) {
    const r = btn.getBoundingClientRect();
    const vw = document.documentElement.clientWidth;
    const vh = window.innerHeight;
    const m = 8;
    panel.style.minWidth = `${Math.min(r.width, vw - 2 * m)}px`;
    panel.style.maxWidth = `${vw - 2 * m}px`;
    panel.style.maxHeight = '';
    const w = panel.offsetWidth;
    const h = panel.scrollHeight;
    let left = align === 'end' ? r.right - w : r.left;
    left = Math.max(m, Math.min(left, vw - m - w));
    const below = vh - r.bottom - m - 4;
    const above = r.top - m - 4;
    let top;
    if (h <= below || below >= above) {
        top = r.bottom + 4;
        panel.style.maxHeight = `${below}px`;
    } else {
        top = r.top - 4 - Math.min(h, above);
        panel.style.maxHeight = `${above}px`;
    }
    panel.style.left = `${left}px`;
    panel.style.top = `${top}px`;
}

function closeOpen(refocus) {
    if (!open) return;
    const { panel, btn, wrap } = open;
    open = null;
    panel.remove();
    btn.setAttribute('aria-expanded', 'false');
    wrap.classList.remove('open');
    if (refocus) btn.focus();
}

document.addEventListener('pointerdown', (e) => {
    if (open && !open.panel.contains(e.target) && !open.wrap.contains(e.target)) closeOpen(false);
}, true);
window.addEventListener('resize', () => closeOpen(false));
document.addEventListener('scroll', (e) => {
    if (open && !open.panel.contains(e.target)) place(open.panel, open.btn, open.align);
}, true);

/** Plain option list, optgroups become small headers. */
function listPanel({ sel, value, choose, close, panel }) {
    panel.classList.add('cs-list');
    const list = document.createElement('div');
    list.setAttribute('role', 'listbox');
    list.tabIndex = -1;
    const items = [];

    const add = (o) => {
        const [text, meta] = optionParts(o);
        const it = document.createElement('div');
        it.className = 'cs-option';
        it.id = `cs-opt-${++uid}`;
        it.setAttribute('role', 'option');
        it.innerHTML = '<span class="cs-text"></span><span class="cs-meta"></span>';
        it.children[0].textContent = text;
        it.children[1].textContent = meta;
        const selected = o.value === value;
        it.classList.toggle('selected', selected);
        it.setAttribute('aria-selected', String(selected));
        if (o.disabled) it.classList.add('disabled');
        it.addEventListener('click', () => { if (!o.disabled) choose(o.value); });
        it.addEventListener('pointermove', () => setActive(items.findIndex(x => x.it === it), false));
        list.appendChild(it);
        items.push({ it, o, text: text.toLowerCase() });
    };
    for (const node of sel.children) {
        if (node.tagName === 'OPTGROUP') {
            const h = document.createElement('div');
            h.className = 'cs-group';
            h.textContent = node.label;
            list.appendChild(h);
            for (const o of node.children) add(o);
        } else {
            add(node);
        }
    }

    let active = -1;
    const setActive = (i, scroll = true) => {
        if (i < 0 || i >= items.length || i === active) return;
        if (active >= 0) items[active].it.classList.remove('active');
        active = i;
        items[i].it.classList.add('active');
        list.setAttribute('aria-activedescendant', items[i].it.id);
        if (scroll) items[i].it.scrollIntoView({ block: 'nearest' });
    };
    const step = (from, dir) => {
        for (let i = from + dir; i >= 0 && i < items.length; i += dir) {
            if (!items[i].o.disabled) return i;
        }
        return from;
    };

    let typed = '';
    let typedAt = 0;
    list.addEventListener('keydown', (e) => {
        switch (e.key) {
            case 'ArrowDown': setActive(step(active, 1)); break;
            case 'ArrowUp': setActive(step(active, -1)); break;
            case 'Home': setActive(step(-1, 1)); break;
            case 'End': setActive(step(items.length, -1)); break;
            case 'PageDown': setActive(Math.min(items.length - 1, active + 8)); break;
            case 'PageUp': setActive(Math.max(0, active - 8)); break;
            case 'Enter':
            case ' ':
                if (active >= 0 && !items[active].o.disabled) choose(items[active].o.value);
                break;
            case 'Escape':
            case 'Tab':
                close();
                break;
            default:
                if (e.key.length !== 1 || e.ctrlKey || e.metaKey || e.altKey) return;
                typed = (Date.now() - typedAt > 700 ? '' : typed) + e.key.toLowerCase();
                typedAt = Date.now();
                setActive(items.findIndex(x => !x.o.disabled && x.text.startsWith(typed)));
        }
        e.preventDefault();
    });
    panel.appendChild(list);

    const start = items.findIndex(x => x.o.value === value);
    return {
        focus() {
            list.focus();
            setActive(start >= 0 ? start : step(-1, 1));
        }
    };
}

/**
 * Replaces the look of one <select>. opts.panel(ctx) can render its own popover content and
 * return { focus() }, opts.label(sel, option) gives [text, meta] for the button.
 */
export function enhanceSelect(sel, opts = {}) {
    if (sel.dataset.cs) return;
    sel.dataset.cs = '1';

    const wrap = document.createElement('div');
    wrap.className = 'cs';
    sel.parentNode.insertBefore(wrap, sel);
    wrap.appendChild(sel);
    sel.classList.add('cs-native');
    sel.tabIndex = -1;
    sel.setAttribute('aria-hidden', 'true');

    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'cs-button';
    btn.setAttribute('aria-haspopup', 'listbox');
    btn.setAttribute('aria-expanded', 'false');
    btn.innerHTML = '<span class="cs-text"></span><span class="cs-meta"></span><span class="cs-chevron" aria-hidden="true"></span>';
    const [textEl, metaEl] = btn.children;
    textEl.id = `cs-text-${++uid}`;
    const label = sel.id ? document.querySelector(`label[for="${sel.id}"]`) : null;
    if (label) {
        if (!label.id) label.id = `${sel.id}-label`;
        btn.setAttribute('aria-labelledby', `${label.id} ${textEl.id}`);
    }
    wrap.appendChild(btn);

    const sync = () => {
        const opt = sel.options[nativeIndex.get.call(sel)];
        const [text, meta] = opts.label ? opts.label(sel, opt) : optionParts(opt);
        textEl.textContent = text;
        metaEl.textContent = meta;
        btn.disabled = sel.disabled;
    };

    // code elsewhere sets .value directly, which fires no event
    Object.defineProperty(sel, 'value', {
        configurable: true,
        get() { return nativeValue.get.call(this); },
        set(v) { nativeValue.set.call(this, v); sync(); },
    });
    Object.defineProperty(sel, 'selectedIndex', {
        configurable: true,
        get() { return nativeIndex.get.call(this); },
        set(v) { nativeIndex.set.call(this, v); sync(); },
    });
    sel.addEventListener('change', sync);
    sel.addEventListener('focus', () => btn.focus());
    new MutationObserver(sync).observe(sel, { childList: true, subtree: true, attributes: true, attributeFilter: ['disabled'] });

    const choose = (v) => {
        if (v !== nativeValue.get.call(sel)) {
            nativeValue.set.call(sel, v);
            sync();
            sel.dispatchEvent(new Event('input', { bubbles: true }));
            sel.dispatchEvent(new Event('change', { bubbles: true }));
        }
        closeOpen(true);
    };

    const openPanel = () => {
        closeOpen(false);
        const panel = document.createElement('div');
        panel.className = 'cs-panel';
        document.body.appendChild(panel);
        const ctx = { sel, value: nativeValue.get.call(sel), choose, close: () => closeOpen(true), panel };
        const handle = (opts.panel || listPanel)(ctx);
        open = { panel, btn, wrap, align: opts.align };
        btn.setAttribute('aria-expanded', 'true');
        wrap.classList.add('open');
        place(panel, btn, opts.align);
        if (handle && handle.focus) handle.focus();
    };

    btn.addEventListener('click', () => {
        if (open && open.btn === btn) closeOpen(true);
        else openPanel();
    });
    btn.addEventListener('keydown', (e) => {
        if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
            e.preventDefault();
            openPanel();
        }
    });

    sync();
}

export function enhanceSelects(selects, custom = {}) {
    selects.forEach(sel => enhanceSelect(sel, custom[sel.id] || {}));
}
