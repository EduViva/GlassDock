(function () {
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const pad2 = (n) => String(n).padStart(2, '0');

  // ---------------------------------------------------------------- dock do topo
  GD.mount($('#hero-dock'), {
    pet: true,
    opts: { max: 1.75 },
    items: [
      { type: 'dots' },
      { type: 'app', icon: 'start', name: 'Iniciar', cls: 'start' },
      { type: 'app', icon: 'folder', name: 'Arquivos', ns: true, on: true },
      { type: 'app', icon: 'browser', name: 'Navegador', on: true },
      { type: 'app', icon: 'chat', name: 'Mensagens', on: true },
      { type: 'app', icon: 'code', name: 'Editor de código', cls: 'hide-sm' },
      { type: 'app', icon: 'photos', name: 'Fotos', cls: 'hide-sm' },
      { type: 'app', icon: 'settings', name: 'Configurações', ns: true, cls: 'hide-md' },
      { type: 'sep', cls: 'hide-sm' },
      { type: 'tile', kind: 'music', units: 3, cls: 'hide-sm' },
      { type: 'tile', kind: 'date', units: 1 },
      { type: 'tile', kind: 'next', units: 3.6, cls: 'hide-md' },
      { type: 'tile', kind: 'act', units: 1, cls: 'hide-sm' },
      { type: 'sep', cls: 'hide-md' },
      { type: 'app', icon: 'downloads', name: 'Downloads', ns: true, cls: 'hide-md' },
      { type: 'app', icon: 'trash', name: 'Lixeira', ns: true, cls: 'hide-md' },
      { type: 'sep' },
      { type: 'tray' },
    ],
  });

  // ---------------------------------------------------------------- dock de navegação
  const nav = $('#navdock');
  GD.mount(nav, {
    opts: { max: 1.6 },
    items: [
      { type: 'app', icon: 'logo', name: 'Início', href: '#topo', cls: 'hide-sm' },
      { type: 'app', icon: 'widgets', name: 'Widgets', href: '#widgets' },
      { type: 'app', icon: 'sliders', name: 'Personalize', href: '#personalize' },
      { type: 'app', icon: 'bolt', name: 'Leve', href: '#leve', cls: 'hide-sm' },
      { type: 'app', icon: 'tag', name: 'Preço', href: '#preco' },
      { type: 'app', icon: 'gift', name: 'Indique e ganhe', href: '/indique' },
      { type: 'app', icon: 'help', name: 'Perguntas', href: '#faq', cls: 'hide-sm' },
      { type: 'sep' },
      { type: 'tile', kind: 'buy', units: 3.4, html: 'Comprar <small>R$ 8,90</small>', onClick: (e) => (e.preventDefault(), GDBuy.buy()) },
    ],
  });
  const hero = $('.hero');
  const io = new IntersectionObserver(([e]) => nav.classList.toggle('show', !e.isIntersecting), { threshold: 0.12 });
  io.observe(hero);

  // ---------------------------------------------------------------- widgets
  const ring = (pct, color, size = 46, w = 5) => {
    const r = (size - w) / 2;
    const c = 2 * Math.PI * r;
    return `<svg viewBox="0 0 ${size} ${size}"><circle cx="${size / 2}" cy="${size / 2}" r="${r}" fill="none" stroke="rgba(255,255,255,.14)" stroke-width="${w}"/><circle cx="${size / 2}" cy="${size / 2}" r="${r}" fill="none" stroke="${color}" stroke-width="${w}" stroke-linecap="round" stroke-dasharray="${(c * pct) / 100} ${c}"/></svg>`;
  };
  const sun = '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="5" fill="#FFD60A"/><g stroke="#FFD60A" stroke-width="2" stroke-linecap="round"><path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4L7 17M17 7l1.4-1.4"/></g></svg>';
  const cloud = '<svg viewBox="0 0 24 24"><path d="M7 18.5a4.5 4.5 0 0 1-.6-9A6 6 0 0 1 18 8.5a4.9 4.9 0 0 1-.6 10z" fill="#E5E5EA"/></svg>';
  const rain = '<svg viewBox="0 0 24 24"><path d="M7 15a4.5 4.5 0 0 1-.6-9A6 6 0 0 1 18 5a4.9 4.9 0 0 1-.6 10z" fill="#C7C7CC"/><path d="M9 18l-1 3M13 18l-1 3M17 18l-1 3" stroke="#64D2FF" stroke-width="1.8" stroke-linecap="round"/></svg>';
  const petSvg = `<svg viewBox="0 0 64 50" class="w-pet"><path d="M10 30c-6-2-8-10-4-14" fill="none" stroke="#F2A65A" stroke-width="5" stroke-linecap="round"/><rect x="16" y="34" width="5" height="12" rx="2.5" fill="#E8924A"/><rect x="23" y="34" width="5" height="12" rx="2.5" fill="#E8924A"/><rect x="36" y="34" width="5" height="12" rx="2.5" fill="#E8924A"/><rect x="43" y="34" width="5" height="12" rx="2.5" fill="#E8924A"/><ellipse cx="31" cy="30" rx="20" ry="10" fill="#F2A65A"/><ellipse cx="30" cy="33" rx="10" ry="5" fill="#FFF3E6"/><path d="M42 12l2-9 6 7z M52 12l4-8 2 9z" fill="#F2A65A"/><circle cx="50" cy="20" r="10" fill="#F2A65A"/><ellipse cx="54" cy="18.5" rx="1.6" ry="2.2" fill="#2B1A10"/></svg>`;

  const G = {
    apps: 'Apps e arquivos', clock: 'Relógios', cal: 'Calendário', notes: 'Notas e mídia',
    sys: 'Sistema', tools: 'Ferramentas', ai: 'IA', fun: 'Clima e diversão',
  };
  const WIDGETS = [
    { g: 'apps', n: 'Coleção de apps', d: 'Um grupo de apps num tile só, que abre em grade ou lista', h: `<div class="w u1 w-apps"><span style="background:#0A84FF"></span><span style="background:#30D158"></span><span style="background:#FF9F0A"></span><span style="background:#BF5AF2"></span></div>` },
    { g: 'apps', n: 'Favoritos', d: 'Os sites que você abre todo dia, um clique cada', h: `<div class="w u2 w-fav"><span style="background:#FF453A">Y</span><span style="background:#0A84FF">M</span><span style="background:#30D158">N</span></div>` },
    { g: 'apps', n: 'Arquivos', d: 'Downloads ou a pasta que quiser, com a contagem do que chegou', h: `<div class="w u1"><span class="w-badge">3</span>${GD.ICONS.downloads().replace('<svg', '<svg width="40" height="40"')}</div>` },
    { g: 'clock', n: 'Relógio', d: 'A sua hora, em números grandes', h: `<div class="w u2"><span class="big" data-live="time"></span></div>` },
    { g: 'clock', n: 'Analógico', d: 'O mesmo relógio, com ponteiros', h: `<div class="w u1"><svg viewBox="0 0 48 48" width="46" height="46" data-live="analog"><circle cx="24" cy="24" r="22" fill="#fff"/><g stroke="#111" stroke-linecap="round"><line class="hh" x1="24" y1="24" x2="24" y2="13" stroke-width="3"/><line class="mm" x1="24" y1="24" x2="24" y2="7" stroke-width="2"/></g><line class="ss" x1="24" y1="27" x2="24" y2="6" stroke="#FF453A" stroke-width="1.2"/><circle cx="24" cy="24" r="2" fill="#FF453A"/></svg></div>` },
    { g: 'clock', n: 'Relógio mundial', d: 'A hora em outra cidade, com 14 para escolher', h: `<div class="w u2"><div class="col"><span class="sm">Londres</span><span class="big" style="font-size:24px" data-live="london"></span></div></div>` },
    { g: 'clock', n: 'Timer de foco', d: 'Contagem regressiva com o anel de progresso no ícone', h: `<div class="w u1"><div class="w-ring">${ring(64, '#FF9F0A')}<b>16</b></div></div>` },
    { g: 'clock', n: 'Pomodoro', d: 'Foco, pausa curta e pausa longa, com os ciclos do dia', h: `<div class="w u1"><div class="w-ring">${ring(80, '#FF453A')}<b>20</b></div></div>` },
    { g: 'clock', n: 'Contagem regressiva', d: 'Quantos dias faltam para a data que importa', h: `<div class="w u2"><div class="col ctr"><span class="big">12</span><span class="sm">dias para as férias</span></div></div>` },
    { g: 'clock', n: 'Progresso do tempo', d: 'Quanto do dia, da semana, do mês ou do ano já foi', h: `<div class="w u2" style="padding:0 14px"><div class="col" style="width:100%;gap:6px"><span class="sm" style="display:flex;justify-content:space-between"><span>Ano</span><span data-live="year"></span></span><div class="w-bar"><i data-live="yearbar" style="background:linear-gradient(90deg,#5ab8ff,#bf5af2)"></i></div></div></div>` },
    { g: 'cal', n: 'Calendário', v: 6, d: 'Data, hora e o próximo compromisso da sua agenda Google, Outlook ou iCloud', h: `<div style="display:flex;gap:8px"><div class="w u1" style="background:#fff;color:#111;flex-direction:column"><b style="color:#FF453A;font-size:11px;letter-spacing:.04em" data-live="wd"></b><span style="font-size:30px;line-height:1" data-live="day"></span></div><div class="w u2" style="background:linear-gradient(180deg,#3aa0ff,#0a5fd8);justify-content:flex-start;padding:0 12px"><div class="col"><span class="sm" style="opacity:.85">Reunião</span><b style="font-size:17px">em 25 min</b></div></div></div>` },
    { g: 'notes', n: 'Lista de tarefas', d: 'Anote, marque e limpe tarefas direto no painel', h: `<div class="w u2 w-tasks"><div class="done"><i></i>E-mails</div><div><i></i>Enviar proposta</div><div><i></i>Mercado</div></div>` },
    { g: 'notes', n: 'Post-it', d: 'Um recado que fica sempre à vista', h: `<div class="w u2 w-note">Ligar pro dentista</div>` },
    { g: 'notes', n: 'Tocando agora', d: 'Capa e controles do Spotify, do navegador ou de qualquer player', h: `<div class="gdock" style="--u:64px"><div class="gd-tile t-music wide" style="--units:3"><div class="cover"><i></i><i></i><i></i><i></i></div><div class="txt"><b>Tocando agora</b><span>Sua playlist</span></div></div></div>` },
    { g: 'sys', n: 'Bateria', v: 3, d: 'A carga e quanto tempo ela ainda dá', h: `<div class="w u1"><div class="w-ring">${ring(82, '#30D158')}<b>82</b></div></div>` },
    { g: 'sys', n: 'Atividade', v: 3, d: 'Processador e memória ao vivo, em barras, números ou anéis', h: `<div class="gdock" style="--u:64px"><div class="gd-tile t-act" data-live="act"><div class="row"><span><b>CPU</b><b class="v">4%</b></span><i><b style="width:8%"></b></i></div><div class="row mem"><span><b>MEM</b><b class="v">43%</b></span><i><b style="width:43%"></b></i></div></div></div>` },
    { g: 'sys', n: 'Rede', v: 2, d: 'O que está subindo e descendo agora, com gráfico', h: `<div class="w u2 w-net" style="padding:0 12px"><div class="col" style="width:100%"><span class="sm" style="display:flex;justify-content:space-between"><span>↓ 12,4 MB/s</span><span>↑ 1,1</span></span><svg viewBox="0 0 100 24" preserveAspectRatio="none"><path d="M0 20 L10 16 L20 18 L30 9 L40 13 L50 5 L60 11 L70 8 L80 14 L90 6 L100 10" fill="none" stroke="#64D2FF" stroke-width="2"/></svg></div></div>` },
    { g: 'sys', n: 'Layout do teclado', d: 'Mostra o layout atual e troca com um clique', h: `<div class="w u1"><div class="col ctr"><span class="w-kb">PT</span><span class="sm">ABNT2</span></div></div>` },
    { g: 'tools', n: 'Conta-gotas', d: 'Pega qualquer cor da tela em 6 formatos, com atalho opcional', h: `<div class="w u2" style="gap:10px"><span class="w-sw" style="background:#0A84FF"></span><span style="font-weight:600;font-size:15px">#0A84FF</span></div>` },
    { g: 'tools', n: 'Capturas de tela', d: 'As últimas capturas, para arrastar direto aonde precisar', h: `<div class="w u1"><div class="w-shots"><i></i><i></i></div></div>` },
    { g: 'tools', n: 'Beber água', d: 'Um copo por vez, a meta do dia e a semana no cartão', h: `<div class="w u2" style="gap:12px"><div class="w-glass"><i></i></div><div class="col"><b style="font-size:18px">5 de 8</b><span class="sm">copos hoje</span></div></div>` },
    { g: 'ai', n: 'Uso do Claude', v: 3, d: 'Limite de 5 horas, limite semanal e tokens do Claude Code', h: `<div class="w u1"><div class="w-ring">${ring(42, '#FF9F0A')}<b>42</b></div></div>` },
    { g: 'ai', n: 'Uso do Codex', d: 'Limites do plano e tokens gastos no Codex', h: `<div class="w u2" style="padding:0 14px"><div class="col" style="width:100%;gap:6px"><span class="sm" style="display:flex;justify-content:space-between"><span>Semana</span><span>31%</span></span><div class="w-bar"><i style="width:31%;background:#30D158"></i></div></div></div>` },
    { g: 'ai', n: 'Uso do OpenCode', d: 'Custo e tokens por provedor', h: `<div class="w u2"><div class="col ctr"><b style="font-size:20px">US$ 1,24</b><span class="sm">hoje, 182 mil tokens</span></div></div>` },
    { g: 'fun', n: 'Clima', v: 3, wide: 1, d: 'Agora, condições e previsão de 5 dias, sem precisar de chave', h: `<div class="w u3 w-fc"><div>${sun}<b>24°</b>Seg</div><div>${cloud}<b>21°</b>Ter</div><div>${rain}<b>18°</b>Qua</div><div>${sun}<b>25°</b>Qui</div><div>${cloud}<b>22°</b>Sex</div></div>` },
    { g: 'fun', n: 'Mascote', v: 9, wide: 1, d: 'Anda por cima da dock conforme a CPU trabalha. Tem até comida', h: `<div class="w u2">${petSvg}</div>` },
  ];

  const grid = $('#wgrid');
  grid.innerHTML = WIDGETS.map(
    (w) => `<article class="wcard card reveal${w.wide ? ' wide' : ''}" data-g="${w.g}"><div class="stage">${w.h}</div><div class="meta"><h3>${w.n}${w.v ? `<span class="var">${w.v} estilos</span>` : ''}</h3><p>${w.d}</p></div></article>`
  ).join('');

  const counts = {};
  WIDGETS.forEach((w) => (counts[w.g] = (counts[w.g] || 0) + 1));
  const chips = $('#wfilter');
  chips.innerHTML =
    `<button class="chip" aria-pressed="true" data-g="all">Todos<span class="c">${WIDGETS.length}</span></button>` +
    Object.entries(G).map(([k, v]) => `<button class="chip" aria-pressed="false" data-g="${k}">${v}<span class="c">${counts[k]}</span></button>`).join('');
  chips.addEventListener('click', (e) => {
    const b = e.target.closest('.chip');
    if (!b) return;
    $$('.chip', chips).forEach((c) => c.setAttribute('aria-pressed', c === b));
    $$('.wcard', grid).forEach((c) => c.classList.toggle('hide', b.dataset.g !== 'all' && c.dataset.g !== b.dataset.g));
  });

  const WD = ['DOM', 'SEG', 'TER', 'QUA', 'QUI', 'SEX', 'SÁB'];
  function live() {
    const d = new Date();
    $$('[data-live="time"]').forEach((e) => (e.textContent = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`));
    $$('[data-live="wd"]').forEach((e) => (e.textContent = WD[d.getDay()]));
    $$('[data-live="day"]').forEach((e) => (e.textContent = d.getDate()));
    try {
      const l = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit', timeZone: 'Europe/London' }).format(d);
      $$('[data-live="london"]').forEach((e) => (e.textContent = l));
    } catch {}
    const y0 = new Date(d.getFullYear(), 0, 1);
    const y1 = new Date(d.getFullYear() + 1, 0, 1);
    const p = ((d - y0) / (y1 - y0)) * 100;
    $$('[data-live="year"]').forEach((e) => (e.textContent = Math.floor(p) + '%'));
    $$('[data-live="yearbar"]').forEach((e) => (e.style.width = p + '%'));
    $$('[data-live="analog"]').forEach((s) => {
      const h = ((d.getHours() % 12) + d.getMinutes() / 60) * 30;
      const m = (d.getMinutes() + d.getSeconds() / 60) * 6;
      const sec = d.getSeconds() * 6;
      s.querySelector('.hh').setAttribute('transform', `rotate(${h} 24 24)`);
      s.querySelector('.mm').setAttribute('transform', `rotate(${m} 24 24)`);
      s.querySelector('.ss').setAttribute('transform', `rotate(${sec} 24 24)`);
    });
  }
  live();
  setInterval(live, 1000);
  let cpu = 8;
  setInterval(() => {
    cpu = Math.max(2, Math.min(60, cpu + (Math.random() - 0.5) * 16));
    $$('[data-live="act"]').forEach((t) => {
      const r = t.querySelector('.row');
      r.querySelector('.v').textContent = Math.round(cpu) + '%';
      r.querySelector('i b').style.width = cpu + '%';
    });
  }, 1500);

  // bandeja do bento
  const tray = $('.tray-demo');
  if (tray) {
    const svg = (p) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${p}</svg>`;
    tray.innerHTML =
      svg('<path d="M5 5h14a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-7l-4.5 3.5V17H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"/>') +
      svg('<path d="M7 18.5a4.5 4.5 0 0 1-.6-9A6 6 0 0 1 18 8.5a4.9 4.9 0 0 1-.6 10z"/>') +
      svg('<path d="M3 9.5a13 13 0 0 1 18 0M6 13a8.5 8.5 0 0 1 12 0M9 16.5a4 4 0 0 1 6 0"/><circle cx="12" cy="19.5" r="1" fill="currentColor"/>') +
      svg('<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z"/><path d="M15.5 9a4 4 0 0 1 0 6"/>') +
      '<div class="t"><b data-live="time"></b><span>Hoje</span></div>';
    live();
  }

  // ---------------------------------------------------------------- personalização
  const cfgDock = GD.mount($('#cfg-dock'), {
    opts: { max: 1.7 },
    items: [
      { type: 'dots' },
      { type: 'app', icon: 'start', name: 'Iniciar', cls: 'start' },
      { type: 'app', icon: 'folder', name: 'Arquivos', ns: true, on: true },
      { type: 'app', icon: 'browser', name: 'Navegador', on: true },
      { type: 'app', icon: 'mail', name: 'E-mail' },
      { type: 'app', icon: 'music', name: 'Música', on: true },
      { type: 'app', icon: 'settings', name: 'Configurações', ns: true },
      { type: 'tile', kind: 'date', units: 1 },
      { type: 'app', icon: 'trash', name: 'Lixeira', ns: true },
      { type: 'sep' },
      { type: 'tray' },
    ],
  });
  const screen = $('#screen');
  function fitCfg() {
    const w = screen.clientWidth;
    const vertical = ['left', 'right'].includes(cfgDock.get('pos'));
    const base = vertical ? screen.clientHeight / 13.5 : w / 17.5;
    cfgDock.el.style.setProperty('--u', Math.round(Math.max(22, Math.min(46, base * (cfgDock.get('size') || 1)))) + 'px');
  }
  new ResizeObserver(fitCfg).observe(screen);

  const CONTROLS = [
    { k: 'pos', l: 'Posição', o: [['bottom', 'Embaixo'], ['top', 'Em cima'], ['left', 'Esquerda'], ['right', 'Direita']] },
    { k: 'glass', l: 'Efeito do vidro', o: [['composition', 'Composição'], ['acrylic', 'Acrílico'], ['blur', 'Desfoque'], ['clear', 'Transparente']] },
    { k: 'tint', l: 'Tom', o: [['dark', 'Escuro'], ['light', 'Claro'], ['system', 'Seguir o Windows']] },
    { k: 'hover', l: 'Ao passar o mouse', o: [['magnify', 'Ampliação'], ['scale', 'Escala'], ['none', 'Nada']] },
    { k: 'anim', l: 'Animação de abertura (clique num ícone)', o: [['jump', 'Pulo'], ['pulse', 'Pulso'], ['shake', 'Tremor']] },
    { k: 'ind', l: 'Indicador de aberto', o: [['dot', 'Ponto'], ['dash', 'Traço'], ['ws', 'Cor do workspace']] },
    { k: 'plate', l: 'Fundo dos ícones', o: [['nonsquare', 'Só nos não quadrados'], ['all', 'Em todos'], ['none', 'Nenhum']] },
    { k: 'tray', l: 'Bandeja', o: [['row', 'Em linha'], ['grid', 'Em grade'], ['compact', 'Compacta']] },
  ];
  const PLATES = ['rgba(255,255,255,.92)', '#1C1C22', '#0A84FF', '#FFD60A', '#FF6B8B', '#30D158'];
  const panel = $('#cfg');
  panel.innerHTML =
    CONTROLS.map(
      (c) =>
        `<div class="opt"><span class="lbl">${c.l}</span><div class="seg" data-k="${c.k}">${c.o
          .map(([v, t], i) => `<button type="button" data-v="${v}" aria-pressed="${i === 0}">${t}</button>`)
          .join('')}</div></div>`
    ).join('') +
    `<div class="opt"><span class="lbl">Cor do fundo dos ícones</span><div class="swatches" id="plates">${PLATES.map(
      (c, i) => `<button type="button" style="background:${c}" data-c="${c}" aria-pressed="${i === 0}" aria-label="Cor ${i + 1}"></button>`
    ).join('')}</div></div>` +
    `<div class="opt"><label for="size">Tamanho dos ícones</label><input type="range" id="size" min="0.7" max="1.3" step="0.05" value="1"></div>`;

  const darkOS = matchMedia('(prefers-color-scheme: dark)');
  panel.addEventListener('click', (e) => {
    const b = e.target.closest('.seg button');
    if (b) {
      const seg = b.parentElement;
      $$('button', seg).forEach((x) => x.setAttribute('aria-pressed', x === b));
      let v = b.dataset.v;
      const k = seg.dataset.k;
      if (k === 'tint' && v === 'system') v = darkOS.matches ? 'dark' : 'light';
      cfgDock.set(k, v);
      if (k === 'pos') {
        screen.dataset.pos = v;
        fitCfg();
      }
      return;
    }
    const s = e.target.closest('#plates button');
    if (s) {
      $$('#plates button').forEach((x) => x.setAttribute('aria-pressed', x === s));
      cfgDock.set('plateColor', s.dataset.c);
      if (cfgDock.get('plate') === 'none') {
        const all = $('.seg[data-k="plate"] button[data-v="nonsquare"]');
        all.click();
      }
    }
  });
  $('#size').addEventListener('input', (e) => {
    cfgDock.set('size', parseFloat(e.target.value));
    fitCfg();
  });

  // ---------------------------------------------------------------- entrada suave
  const rio = new IntersectionObserver(
    (es) =>
      es.forEach((e) => {
        if (e.isIntersecting) {
          e.target.classList.add('in');
          rio.unobserve(e.target);
        }
      }),
    { threshold: 0.12, rootMargin: '0px 0px -40px 0px' }
  );
  $$('.reveal').forEach((el) => rio.observe(el));
  $('#yr').textContent = new Date().getFullYear();
})();
