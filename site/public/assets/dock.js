// Dock interativa do site: mesma lógica visual do GlassDock (ampliação, placas,
// indicadores, animações de abertura, widgets vivos, bandeja e mascote).
(function () {
  let uid = 0;
  const id = () => 'g' + ++uid;

  // ---------------------------------------------------------------- ícones
  function squircle(c1, c2, glyph) {
    const g = id();
    return `<svg viewBox="0 0 64 64"><defs><linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${c1}"/><stop offset="1" stop-color="${c2}"/></linearGradient></defs><rect x="1" y="1" width="62" height="62" rx="15" fill="url(#${g})"/><rect x="1.5" y="1.5" width="61" height="61" rx="14.5" fill="none" stroke="#fff" stroke-opacity=".18"/>${glyph}</svg>`;
  }
  const W = 'fill="none" stroke="#fff" stroke-width="3.6" stroke-linecap="round" stroke-linejoin="round"';
  const ICONS = {
    start: () =>
      squircle('#3AA0FF', '#0A5FD8', '<g fill="#fff"><rect x="17" y="17" width="13" height="13" rx="2.5"/><rect x="34" y="17" width="13" height="13" rx="2.5"/><rect x="17" y="34" width="13" height="13" rx="2.5"/><rect x="34" y="34" width="13" height="13" rx="2.5"/></g>'),
    browser: () =>
      squircle('#5AC8FA', '#0A6CFF', `<circle cx="32" cy="32" r="17" ${W}/><path d="M15 32h34M32 15c6 5 6 29 0 34M32 15c-6 5-6 29 0 34" ${W} stroke-width="2.8"/>`),
    chat: () =>
      squircle('#5EE07E', '#1FAE4B', '<path d="M32 16c-10.5 0-19 6.9-19 15.4 0 4.8 2.7 9 7 11.9L18 50l8.4-4.2c1.8.4 3.7.7 5.6.7 10.5 0 19-6.9 19-15.5S42.5 16 32 16z" fill="#fff"/>'),
    music: () =>
      squircle('#FF6B8B', '#FF2D55', '<path d="M27 43V21l17-4v22" fill="none" stroke="#fff" stroke-width="3.6" stroke-linejoin="round"/><circle cx="23" cy="44" r="5.5" fill="#fff"/><circle cx="40" cy="40" r="5.5" fill="#fff"/>'),
    terminal: () =>
      squircle('#3A3A42', '#16161B', '<path d="M18 23l9 8-9 8" fill="none" stroke="#30D158" stroke-width="3.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M31 42h14" stroke="#fff" stroke-width="3.8" stroke-linecap="round"/>'),
    code: () =>
      squircle('#8E7CFF', '#5E3BEE', `<path d="M25 22l-10 10 10 10M39 22l10 10-10 10" ${W}/><path d="M35 19l-6 26" ${W} stroke-opacity=".7"/>`),
    photos: () =>
      squircle('#FFD60A', '#FF9F0A', '<circle cx="24" cy="24" r="5" fill="#fff"/><path d="M12 47l13-14 9 9 7-7 11 12z" fill="#fff"/>'),
    mail: () =>
      squircle('#64D2FF', '#0A84FF', '<rect x="14" y="20" width="36" height="25" rx="5" fill="#fff"/><path d="M15 22l17 13 17-13" fill="none" stroke="#0A84FF" stroke-width="3" stroke-linejoin="round"/>'),
    notes: () =>
      squircle('#FFE873', '#FFC800', '<rect x="18" y="16" width="28" height="33" rx="4" fill="#fff"/><path d="M23 26h18M23 33h18M23 40h11" stroke="#E0A800" stroke-width="3" stroke-linecap="round"/>'),
    // ícones sem fundo quadrado (é neles que a placa aparece)
    folder: () => {
      const g = id();
      return `<svg viewBox="0 0 64 64"><defs><linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFD866"/><stop offset="1" stop-color="#F5B000"/></linearGradient></defs><path d="M6 17a5 5 0 0 1 5-5h14l6 6h22a5 5 0 0 1 5 5v4H6z" fill="#E09A00"/><rect x="6" y="22" width="52" height="32" rx="5" fill="url(#${g})"/><rect x="6.5" y="22.5" width="51" height="31" rx="4.5" fill="none" stroke="#fff" stroke-opacity=".35"/></svg>`;
    },
    downloads: () => {
      const g = id();
      return `<svg viewBox="0 0 64 64"><defs><linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#6CC4FF"/><stop offset="1" stop-color="#1A84F0"/></linearGradient></defs><path d="M6 17a5 5 0 0 1 5-5h14l6 6h22a5 5 0 0 1 5 5v4H6z" fill="#1366C8"/><rect x="6" y="22" width="52" height="32" rx="5" fill="url(#${g})"/><path d="M32 29v15M25 37l7 7 7-7" fill="none" stroke="#fff" stroke-width="3.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
    },
    settings: () => {
      const g = id();
      const teeth = Array.from({ length: 8 }, (_, i) => `<rect x="28.5" y="6" width="7" height="12" rx="2" transform="rotate(${i * 45} 32 32)"/>`).join('');
      return `<svg viewBox="0 0 64 64"><defs><linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#C7C7CC"/><stop offset="1" stop-color="#7C7C82"/></linearGradient></defs><g fill="url(#${g})">${teeth}<circle cx="32" cy="32" r="20"/></g><circle cx="32" cy="32" r="8" fill="#3A3A40"/></svg>`;
    },
    trash: () =>
      '<svg viewBox="0 0 64 64"><path d="M15 18h34" stroke="#D1D1D6" stroke-width="4" stroke-linecap="round"/><path d="M26 18v-4h12v4" fill="none" stroke="#D1D1D6" stroke-width="3.6" stroke-linejoin="round"/><path d="M18 22h28l-3 30a4 4 0 0 1-4 3.6H25a4 4 0 0 1-4-3.6z" fill="#E5E5EA" fill-opacity=".88" stroke="#fff" stroke-opacity=".6"/><path d="M27 29v19M32 29v19M37 29v19" stroke="#8E8E93" stroke-width="2.4" stroke-linecap="round"/></svg>',
    // ícones da navegação do site
    logo: () =>
      squircle('#5AB8FF', '#5E3BEE', '<rect x="9" y="38" width="46" height="15" rx="6.5" fill="#fff" fill-opacity=".6" stroke="#fff" stroke-opacity=".85" stroke-width="1.2"/><rect x="13" y="28" width="11" height="11" rx="3" fill="#FFD60A"/><rect x="26.5" y="24.5" width="11" height="14.5" rx="3" fill="#FF6B8B"/><rect x="40" y="28" width="11" height="11" rx="3" fill="#30D158"/>'),
    widgets: () =>
      squircle('#FF9F0A', '#FF6A00', '<g fill="#fff"><rect x="15" y="15" width="15" height="15" rx="4"/><rect x="34" y="15" width="15" height="15" rx="7.5"/><rect x="15" y="34" width="34" height="15" rx="4"/></g>'),
    sliders: () =>
      squircle('#BF5AF2', '#8A2BE2', `<path d="M18 22h28M18 32h28M18 42h28" ${W} stroke-opacity=".55"/><circle cx="26" cy="22" r="4.5" fill="#fff"/><circle cx="39" cy="32" r="4.5" fill="#fff"/><circle cx="30" cy="42" r="4.5" fill="#fff"/>`),
    bolt: () => squircle('#5EE07E', '#1FAE4B', '<path d="M35 13L19 36h12l-3 15 17-24H33z" fill="#fff"/>'),
    tag: () =>
      squircle('#64D2FF', '#0A84FF', '<path d="M15 31V17a2 2 0 0 1 2-2h14l18 18-16 16z" fill="#fff"/><circle cx="23" cy="23" r="3.5" fill="#0A84FF"/>'),
    gift: () =>
      squircle('#FF6B8B', '#FF2D55', '<rect x="15" y="27" width="34" height="22" rx="3" fill="#fff"/><rect x="13" y="21" width="38" height="8" rx="2.5" fill="#fff"/><path d="M32 21v28" stroke="#FF2D55" stroke-width="3.4"/><path d="M32 21c-3-7-12-8-12-3s9 3 12 3c3 0 12 2 12-3s-9-4-12 3z" fill="none" stroke="#fff" stroke-width="3"/>'),
    help: () =>
      squircle('#8E8E93', '#55555B', '<path d="M25 25a7 7 0 1 1 10 6.3c-2 1-3 2.4-3 4.7v1" fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round"/><circle cx="32" cy="45" r="2.8" fill="#fff"/>'),
  };

  const TRAY = {
    wifi: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M3 9.5a13 13 0 0 1 18 0M6 13a8.5 8.5 0 0 1 12 0M9 16.5a4 4 0 0 1 6 0"/><circle cx="12" cy="19.5" r="1.2" fill="currentColor"/></svg>',
    vol: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11"/></svg>',
    bat: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="2.5" y="7" width="17" height="10" rx="2.6"/><rect x="4.6" y="9.1" width="10.5" height="5.8" rx="1.2" fill="currentColor" stroke="none"/><path d="M21.5 10.5v3" stroke-linecap="round"/></svg>',
    cloud: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M7 18.5a4.5 4.5 0 0 1-.6-9A6 6 0 0 1 18 8.5a4.9 4.9 0 0 1-.6 10z" opacity=".9"/></svg>',
    chatb: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M5 5h14a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-7l-4.5 3.5V17H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z" opacity=".9"/></svg>',
  };

  const SUN = '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="5" fill="#FFD60A"/><g stroke="#FFD60A" stroke-width="2" stroke-linecap="round"><path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8"/></g></svg>';
  const CTL = {
    prev: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M11 12l8-6v12zM4 12l7-6v12z" opacity=".9"/></svg>',
    pause: '<svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1.2"/><rect x="14" y="5" width="4" height="14" rx="1.2"/></svg>',
    next: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M13 12L5 6v12zM20 12l-7-6v12z" opacity=".9"/></svg>',
  };

  const PET = `<svg viewBox="0 0 64 50">
    <text class="zz" x="46" y="8">z</text>
    <path class="tail" d="M10 30c-6-2-8-10-4-14" fill="none" stroke="#F2A65A" stroke-width="5" stroke-linecap="round"/>
    <rect class="leg a" x="16" y="34" width="5" height="12" rx="2.5" fill="#E8924A"/>
    <rect class="leg b" x="23" y="34" width="5" height="12" rx="2.5" fill="#E8924A"/>
    <rect class="leg b" x="36" y="34" width="5" height="12" rx="2.5" fill="#E8924A"/>
    <rect class="leg a" x="43" y="34" width="5" height="12" rx="2.5" fill="#E8924A"/>
    <ellipse cx="31" cy="30" rx="20" ry="10" fill="#F2A65A"/>
    <ellipse cx="30" cy="33" rx="10" ry="5" fill="#FFF3E6"/>
    <path d="M42 12l2-9 6 7z M52 12l4-8 2 9z" fill="#F2A65A"/>
    <circle cx="50" cy="20" r="10" fill="#F2A65A"/>
    <path d="M44.5 8.5l1.2-3.6 2.6 3.2z M53.3 8.6l2-3.4 .8 3.9z" fill="#FFB3C1"/>
    <ellipse class="eye" cx="54" cy="18.5" rx="1.6" ry="2.2" fill="#2B1A10"/>
    <ellipse cx="58.5" cy="22.5" rx="1.5" ry="1" fill="#FF8FA3"/>
  </svg>`;

  // ---------------------------------------------------------------- helpers
  const pad2 = (n) => String(n).padStart(2, '0');
  const WD = ['DOM', 'SEG', 'TER', 'QUA', 'QUI', 'SEX', 'SÁB'];
  const MON = ['jan', 'fev', 'mar', 'abr', 'mai', 'jun', 'jul', 'ago', 'set', 'out', 'nov', 'dez'];
  const hhmm = (d) => `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  const nextAt = new Date(Date.now() + 25 * 60000);

  function tile(kind, units) {
    const t = document.createElement('div');
    t.className = 'gd-tile t-' + kind + (units > 1 ? ' wide' : '');
    t.style.setProperty('--units', units);
    if (kind === 'date') t.innerHTML = '<span class="wd"></span><span class="d"></span>';
    if (kind === 'music')
      t.innerHTML = `<div class="cover"><i></i><i></i><i></i><i></i></div><div class="txt"><b>Tocando agora</b><span>Sua playlist favorita</span></div><div class="ctl">${CTL.pause}${CTL.next}</div>`;
    if (kind === 'next')
      t.innerHTML = '<span class="time"></span><div class="col"><span class="day"></span><b>Reunião <em class="rel"></em></b></div>';
    if (kind === 'act')
      t.innerHTML = '<div class="row"><span><b>CPU</b><em class="v">3%</em></span><i><b></b></i></div><div class="row mem"><span><b>MEM</b><em class="v">41%</em></span><i><b></b></i></div>';
    if (kind === 'weather') t.innerHTML = SUN + '<b>24°</b>';
    t.querySelectorAll('em.v').forEach((e) => (e.style.fontStyle = 'normal'));
    t.querySelectorAll('.row span b').forEach((e) => (e.style.fontWeight = '600'));
    return t;
  }

  function mount(root, cfg) {
    const opts = Object.assign(
      { pos: 'bottom', glass: 'composition', tint: 'dark', hover: 'magnify', ind: 'dot', plate: 'nonsquare', anim: 'jump', tray: 'row', max: 1.7, u: null },
      cfg.opts || {}
    );
    const dock = document.createElement('div');
    dock.className = 'gdock';
    dock.innerHTML = '<div class="gd-bar"></div><div class="gd-items"></div>';
    const list = dock.querySelector('.gd-items');

    for (const it of cfg.items) {
      if (it.type === 'app') {
        const b = document.createElement(it.href ? 'a' : 'button');
        b.className = 'gd-app' + (it.ns ? ' ns' : '') + (it.on ? ' on' : '') + (it.cls ? ' ' + it.cls : '');
        if (it.href) b.href = it.href;
        else b.type = 'button';
        b.setAttribute('aria-label', it.name);
        b.innerHTML = `<span class="gd-plate"></span><span class="gd-ico">${ICONS[it.icon]()}</span><i class="gd-ind"></i><span class="gd-tip">${it.name}</span>`;
        list.appendChild(b);
      } else if (it.type === 'tile') {
        const t = tile(it.kind, it.units || 1);
        if (it.kind === 'buy') {
          const a = document.createElement('a');
          a.className = t.className + (it.cls ? ' ' + it.cls : '');
          a.style.cssText = t.style.cssText;
          a.href = it.href || '#comprar';
          a.innerHTML = it.html || 'Comprar <small>R$ 8,90</small>';
          if (it.onClick) a.addEventListener('click', it.onClick);
          list.appendChild(a);
        } else {
          if (it.cls) t.className += ' ' + it.cls;
          list.appendChild(t);
        }
      } else if (it.type === 'sep') {
        list.insertAdjacentHTML('beforeend', `<div class="gd-sep ${it.cls || ''}"></div>`);
      } else if (it.type === 'dots') {
        list.insertAdjacentHTML(
          'beforeend',
          '<div class="gd-dots" aria-hidden="true"><i class="on" style="--c:#0A84FF"></i><i style="--c:#30D158"></i><i style="--c:#BF5AF2"></i></div>'
        );
      } else if (it.type === 'tray') {
        list.insertAdjacentHTML(
          'beforeend',
          `<div class="gd-tray"><div class="ic">${TRAY.chatb}${TRAY.cloud}${TRAY.wifi}${TRAY.vol}${TRAY.bat}</div><div class="clock"><b class="c-time"></b><span class="c-date"></span></div></div>`
        );
      }
    }
    if (cfg.pet) {
      dock.insertAdjacentHTML('beforeend', `<div class="gd-pet">${PET}</div>`);
    }
    root.appendChild(dock);

    function apply() {
      for (const k of ['pos', 'glass', 'tint', 'hover', 'ind', 'plate', 'anim', 'tray']) dock.dataset[k] = opts[k];
      if (opts.u) dock.style.setProperty('--u', opts.u + 'px');
      if (opts.plateColor) dock.style.setProperty('--plate', opts.plateColor);
      reset();
    }

    // ---------------------------------------------------------------- ampliação
    const apps = () => [...list.querySelectorAll('.gd-app')];
    let raf = 0;
    let last = null;
    function reset() {
      dock.classList.add('still');
      apps().forEach((a) => a.style.setProperty('--s', 1));
    }
    function frame() {
      raf = 0;
      if (!last) return;
      const vertical = opts.pos === 'left' || opts.pos === 'right';
      const u = parseFloat(getComputedStyle(dock).getPropertyValue('--u')) || 52;
      for (const a of apps()) {
        const r = a.getBoundingClientRect();
        const c = vertical ? r.top + r.height / 2 : r.left + r.width / 2;
        const m = vertical ? last.clientY : last.clientX;
        const d = Math.abs(m - c) / u;
        let s = 1;
        if (opts.hover === 'magnify') s = 1 + (opts.max - 1) * Math.exp(-(d * d) / 2.4);
        else if (opts.hover === 'scale') s = a.contains(last.target) ? 1.22 : 1;
        a.style.setProperty('--s', s.toFixed(3));
      }
    }
    dock.addEventListener('pointermove', (e) => {
      if (e.pointerType !== 'mouse' || opts.hover === 'none') return;
      dock.classList.remove('still');
      last = e;
      if (!raf) raf = requestAnimationFrame(frame);
    });
    dock.addEventListener('pointerleave', () => {
      last = null;
      reset();
    });

    // ---------------------------------------------------------------- clique
    list.addEventListener('click', (e) => {
      const a = e.target.closest('.gd-app');
      if (!a || a.tagName === 'A') return;
      const cls = 'anim-' + opts.anim;
      a.classList.remove('anim-jump', 'anim-pulse', 'anim-shake');
      void a.offsetWidth;
      a.classList.add(cls);
      setTimeout(() => {
        a.classList.remove(cls);
        a.classList.add('on');
      }, 900);
    });
    const dots = list.querySelector('.gd-dots');
    if (dots)
      dots.addEventListener('click', (e) => {
        const i = e.target.closest('i');
        if (!i) return;
        dots.querySelectorAll('i').forEach((x) => x.classList.toggle('on', x === i));
        dock.style.setProperty('--ws', i.style.getPropertyValue('--c'));
        const idx = [...dots.children].indexOf(i);
        apps().forEach((a, k) => {
          if (a.classList.contains('start')) return;
          a.classList.toggle('on', (k + idx) % 3 !== 0);
        });
      });

    // ---------------------------------------------------------------- relógios e atividade
    function tick() {
      const d = new Date();
      dock.querySelectorAll('.c-time').forEach((e) => (e.textContent = hhmm(d)));
      dock.querySelectorAll('.c-date').forEach((e) => (e.textContent = `${pad2(d.getDate())}/${pad2(d.getMonth() + 1)}/${d.getFullYear()}`));
      dock.querySelectorAll('.t-date').forEach((t) => {
        t.querySelector('.wd').textContent = WD[d.getDay()];
        t.querySelector('.d').textContent = d.getDate();
      });
      dock.querySelectorAll('.t-next').forEach((t) => {
        t.querySelector('.time').textContent = hhmm(d);
        t.querySelector('.day').textContent = `${WD[d.getDay()].charAt(0) + WD[d.getDay()].slice(1).toLowerCase()}, ${d.getDate()} de ${MON[d.getMonth()]}`;
        const min = Math.max(1, Math.round((nextAt - d) / 60000));
        t.querySelector('.rel').textContent = nextAt > d ? `em ${min} min` : 'agora';
      });
    }
    let cpu = 6, mem = 41;
    function act() {
      cpu = Math.max(2, Math.min(64, cpu + (Math.random() - 0.5) * 14));
      mem = Math.max(36, Math.min(58, mem + (Math.random() - 0.5) * 3));
      dock.querySelectorAll('.t-act').forEach((t) => {
        const [c, m] = t.querySelectorAll('.row');
        c.querySelector('.v').textContent = Math.round(cpu) + '%';
        c.querySelector('i b').style.width = cpu + '%';
        m.querySelector('.v').textContent = Math.round(mem) + '%';
        m.querySelector('i b').style.width = mem + '%';
      });
    }
    tick();
    act();
    setInterval(tick, 10000);
    if (dock.querySelector('.t-act')) setInterval(act, 1600);

    // ---------------------------------------------------------------- mascote
    const pet = dock.querySelector('.gd-pet');
    if (pet) {
      let x = 12;
      const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
      const step = () => {
        const roll = Math.random();
        pet.classList.remove('walk', 'sleep');
        if (reduce || roll < 0.25) {
          if (roll < 0.12 && !reduce) pet.classList.add('sleep');
          setTimeout(step, 3500 + Math.random() * 3000);
          return;
        }
        const to = 4 + Math.random() * 84;
        const dur = Math.abs(to - x) * 90 + 400;
        pet.classList.toggle('flip', to < x);
        pet.classList.add('walk');
        pet.style.transitionDuration = dur + 'ms';
        pet.style.left = to + '%';
        x = to;
        setTimeout(step, dur + 600 + Math.random() * 1800);
      };
      pet.style.left = x + '%';
      setTimeout(step, 1200);
    }

    apply();
    return {
      el: dock,
      set(k, v) {
        opts[k] = v;
        apply();
      },
      get: (k) => opts[k],
    };
  }

  window.GD = { mount, ICONS };
})();
