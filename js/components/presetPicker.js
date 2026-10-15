/**
 * Preset popover: rows are bit ranges, columns are the preset groups. Narrow screens get the
 * rows only, with the presets of a row wrapping.
 */
import { presetGroups, getPresetById } from '../templates/presets.js';

const fmt = (bpw) => bpw.toFixed(2);

export function presetLabel(sel, opt) {
    const p = opt ? getPresetById(opt.value) : null;
    if (!p) return ['Custom', ''];
    return [p.label, `${fmt(p.bpw)} bpw`];
}

const squash = (s) => s.toLowerCase().replace(/[\s_-]/g, '');

export function presetPanel({ value, choose, close, panel }) {
    panel.classList.add('pp');

    const all = [];
    presetGroups.forEach((group, col) => {
        Object.entries(group.presets).forEach(([id, p]) => {
            const full = p.label || id;
            all.push({ id, full, label: full.replace(/\s*\(.*\)$/, ''), bpw: p.bpw, col, group });
        });
    });
    all.sort((a, b) => a.bpw - b.bpw);
    const rows = [...new Set(all.map(p => Math.floor(p.bpw)))];

    const search = document.createElement('input');
    search.type = 'search';
    search.className = 'pp-search';
    search.placeholder = 'Find a preset, e.g. iq4 or 4.5';
    search.setAttribute('aria-label', 'Find a preset');
    search.autocomplete = 'off';
    search.spellcheck = false;
    panel.appendChild(search);

    const compact = document.documentElement.clientWidth < 600;
    const grid = document.createElement('div');
    grid.className = compact ? 'pp-grid compact' : 'pp-grid';
    if (!compact) grid.style.gridTemplateColumns = `2.25rem repeat(${presetGroups.length}, minmax(4.4rem, 1fr))`;

    const items = [];
    const item = (p) => {
        const b = document.createElement('button');
        b.type = 'button';
        b.className = 'pp-item';
        b.tabIndex = -1;
        b.title = `${p.full}\n${p.group.label}, ${fmt(p.bpw)} bits per weight`;
        b.innerHTML = '<span class="pp-name"></span><span class="pp-bpw"></span>';
        b.children[0].textContent = p.label;
        b.children[1].textContent = fmt(p.bpw);
        if (p.id === value) {
            b.classList.add('selected');
            b.setAttribute('aria-current', 'true');
        }
        b.addEventListener('click', () => choose(p.id));
        items.push({ b, p, key: squash(p.full), num: fmt(p.bpw) });
        return b;
    };
    const rowLabel = (bits) => {
        const d = document.createElement('div');
        d.className = 'pp-bits';
        d.textContent = bits;
        d.title = `${bits} to ${bits + 1} bits per weight`;
        return d;
    };

    if (!compact) {
        const corner = document.createElement('div');
        corner.className = 'pp-corner';
        corner.textContent = 'bpw';
        grid.appendChild(corner);
        presetGroups.forEach(group => {
            const h = document.createElement('div');
            h.className = 'pp-col';
            h.textContent = group.short;
            h.title = group.label;
            grid.appendChild(h);
        });
        rows.forEach(bits => {
            grid.appendChild(rowLabel(bits));
            presetGroups.forEach((_, col) => {
                const cell = document.createElement('div');
                cell.className = 'pp-cell';
                all.filter(p => p.col === col && Math.floor(p.bpw) === bits).forEach(p => cell.appendChild(item(p)));
                grid.appendChild(cell);
            });
        });
    } else {
        rows.forEach(bits => {
            const row = document.createElement('div');
            row.className = 'pp-row';
            row.appendChild(rowLabel(bits));
            const cell = document.createElement('div');
            cell.className = 'pp-cell';
            all.filter(p => Math.floor(p.bpw) === bits).forEach(p => cell.appendChild(item(p)));
            row.appendChild(cell);
            grid.appendChild(row);
        });
    }
    panel.appendChild(grid);

    const live = () => items.filter(x => !x.b.classList.contains('dim'));

    search.addEventListener('input', () => {
        const q = search.value.trim().toLowerCase();
        const sq = squash(q);
        items.forEach(x => {
            const hit = !q || x.key.includes(sq) || x.num.startsWith(q) || squash(x.p.group.short) === sq;
            x.b.classList.toggle('dim', !hit);
        });
    });

    // arrow keys move to the closest preset in that direction
    const move = (from, dir) => {
        const a = from.getBoundingClientRect();
        const ax = a.left + a.width / 2;
        const ay = a.top + a.height / 2;
        let best = null;
        let bestScore = Infinity;
        for (const { b } of live()) {
            if (b === from) continue;
            const r = b.getBoundingClientRect();
            const dx = r.left + r.width / 2 - ax;
            const dy = r.top + r.height / 2 - ay;
            let score;
            if (dir === 'right') score = dx > 2 ? dx + 4 * Math.abs(dy) : Infinity;
            else if (dir === 'left') score = dx < -2 ? -dx + 4 * Math.abs(dy) : Infinity;
            else if (dir === 'down') score = dy > 2 ? dy + 2 * Math.abs(dx) : Infinity;
            else score = dy < -2 ? -dy + 2 * Math.abs(dx) : Infinity;
            if (score < bestScore) {
                bestScore = score;
                best = b;
            }
        }
        return best;
    };
    const first = () => {
        const l = live();
        return (!search.value && l.find(x => x.p.id === value)) || l[0];
    };

    search.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowDown') {
            const f = first();
            if (f) f.b.focus();
        } else if (e.key === 'Enter') {
            const f = first();
            if (f) choose(f.p.id);
        } else if (e.key === 'Escape') {
            if (search.value) {
                search.value = '';
                search.dispatchEvent(new Event('input'));
            } else {
                close();
            }
        } else if (e.key === 'Tab') {
            close();
        } else {
            return;
        }
        e.preventDefault();
    });

    grid.addEventListener('keydown', (e) => {
        const dirs = { ArrowRight: 'right', ArrowLeft: 'left', ArrowDown: 'down', ArrowUp: 'up' };
        if (dirs[e.key]) {
            const next = move(e.target, dirs[e.key]);
            if (next) next.focus();
            else if (dirs[e.key] === 'up') search.focus();
        } else if (e.key === 'Escape' || e.key === 'Tab') {
            close();
        } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && e.key !== ' ') {
            search.focus();
            return;
        } else {
            return;
        }
        e.preventDefault();
    });

    return {
        focus() {
            search.focus();
            const sel = items.find(x => x.p.id === value);
            if (sel) sel.b.scrollIntoView({ block: 'nearest' });
        }
    };
}
