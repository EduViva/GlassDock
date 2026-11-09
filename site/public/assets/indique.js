(function () {
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const COMMISSION = 445;
  const host = location.host && !/^(localhost|127\.)/.test(location.hostname) ? location.host : 'glassdock.com.br';
  const brl = (c) => (c / 100).toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' });
  const store = {
    get: (k) => { try { return localStorage.getItem(k); } catch { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch {} },
    del: (k) => { try { localStorage.removeItem(k); } catch {} },
  };

  function toast(msg) {
    const t = $('#toast');
    t.textContent = msg;
    t.classList.add('show');
    clearTimeout(t._h);
    t._h = setTimeout(() => t.classList.remove('show'), 3200);
  }
  async function copy(text, msg) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement('textarea');
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      ta.remove();
    }
    toast(msg || 'Copiado');
  }

  $$('[data-host]').forEach((e) => (e.textContent = host));

  // ---------------------------------------------------------------- link digitando
  const names = ['ana', 'joao', 'bia', 'lucas', 'mari', 'seunome'];
  const typed = $('#typed');
  let ni = 0;
  async function typeLoop() {
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    for (;;) {
      await wait(1800);
      for (let i = typed.textContent.length; i >= 0; i--) {
        typed.textContent = typed.textContent.slice(0, i);
        await wait(55);
      }
      ni = (ni + 1) % names.length;
      for (const ch of names[ni]) {
        typed.textContent += ch;
        await wait(95);
      }
    }
  }
  typeLoop();

  // ---------------------------------------------------------------- simulador
  const people = $('#people');
  const upd = () => {
    $('#people-n').textContent = people.value;
    $('#earn').textContent = brl(people.value * COMMISSION);
  };
  people.addEventListener('input', upd);
  upd();

  // ---------------------------------------------------------------- formulário
  const PH = { CPF: '000.000.000-00', EMAIL: 'voce@email.com', PHONE: '(11) 99999-9999', CNPJ: '00.000.000/0000-00', RANDOM: 'chave aleatória' };
  $('#f-pixtype').addEventListener('change', (e) => ($('#f-pix').placeholder = PH[e.target.value]));
  $('#f-slug').addEventListener('input', (e) => {
    const v = e.target.value.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9-]/g, '');
    if (v !== e.target.value) e.target.value = v;
  });
  $('#have-key').addEventListener('click', (e) => {
    e.preventDefault();
    $('#ref-form').hidden = true;
    $('#key-form').hidden = false;
    $('#f-key').focus();
  });
  $('#no-key').addEventListener('click', (e) => {
    e.preventDefault();
    $('#ref-form').hidden = false;
    $('#key-form').hidden = true;
  });

  $('#ref-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const f = e.target;
    const err = $('#f-error');
    err.textContent = '';
    const body = Object.fromEntries(new FormData(f));
    if ((body.name || '').trim().length < 2) return (err.textContent = 'Escreva seu nome');
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(body.email || '')) return (err.textContent = 'Confira o e-mail');
    if (!(body.pixKey || '').trim()) return (err.textContent = 'Coloque a chave PIX');
    const btn = f.querySelector('button[type=submit]');
    btn.classList.add('loading');
    try {
      const r = await fetch('/api/ref', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
      const d = await r.json().catch(() => ({}));
      if (!r.ok) throw new Error(d.error || 'Não deu certo agora. Tente de novo');
      store.set('gd_ref_key', d.key);
      await loadPanel(d.key, true);
    } catch (ex) {
      err.textContent = /fetch|JSON/i.test(ex.message) ? 'Sem conexão com o servidor agora. Tente de novo' : ex.message;
    } finally {
      btn.classList.remove('loading');
    }
  });

  $('#key-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const k = $('#f-key').value.trim().toLowerCase();
    $('#k-error').textContent = '';
    if (!/^[a-z0-9]{20}$/.test(k)) return ($('#k-error').textContent = 'A chave tem 20 letras e números');
    try {
      await loadPanel(k, true);
      store.set('gd_ref_key', k);
    } catch (ex) {
      $('#k-error').textContent = ex.message;
    }
  });

  // ---------------------------------------------------------------- painel
  async function loadPanel(key, scroll) {
    const r = await fetch('/api/ref?k=' + encodeURIComponent(key));
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.error || 'Chave não encontrada');
    const link = `${host}/${d.slug}`;
    const full = `${location.protocol}//${location.host}/${d.slug}`;
    $('#p-initial').textContent = d.name.charAt(0).toUpperCase();
    $('#p-hello').textContent = `Oi, ${d.name}`;
    $('#p-link').textContent = link;
    $('#p-key').textContent = key;
    $('#p-pix').textContent = d.pix.key;
    $('#s-clicks').textContent = d.clicks;
    $('#s-sales').textContent = d.sales;
    $('#s-pending').textContent = brl(d.pending);
    $('#s-paid').textContent = brl(d.paid);
    $('#p-list').innerHTML = d.items.length
      ? d.items
          .slice(0, 12)
          .map((it) => `<div class="it"><span>${new Date(it.createdAt).toLocaleDateString('pt-BR')} · ${brl(it.amount)}</span><span class="st ${it.status}">${it.status === 'paga' ? 'paga' : 'a receber'}</span></div>`)
          .join('')
      : '<p class="hint">Quando alguém comprar pelo seu link, a venda aparece aqui</p>';

    const msg = `Achei o GlassDock: a barra de tarefas do Windows vira uma dock de vidro com widgets. Custa R$ 8,90 uma vez só ${full}`;
    $('#sh-wa').href = 'https://wa.me/?text=' + encodeURIComponent(msg);
    $('#sh-tg').href = `https://t.me/share/url?url=${encodeURIComponent(full)}&text=${encodeURIComponent(msg.replace(' ' + full, ''))}`;
    $('#sh-x').href = 'https://x.com/intent/post?text=' + encodeURIComponent(msg);
    if (navigator.share) {
      $('#sh-native').hidden = false;
      $('#sh-native').onclick = () => navigator.share({ title: 'GlassDock', text: msg.replace(' ' + full, ''), url: full }).catch(() => {});
    }
    $('#copy-link').onclick = () => copy(full, 'Link copiado');
    $('#copy-key').onclick = () => copy(`${location.origin}/indique?k=${key}`, 'Link do painel copiado');

    $('#form-card').hidden = true;
    $('#painel').hidden = false;
    $('#nav-panel').hidden = false;
    $('#nav-create').textContent = 'Copiar meu link';
    $('#nav-create').onclick = (e) => (e.preventDefault(), copy(full, 'Link copiado'));
    if (scroll) $('#criar').scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  $('#logout').addEventListener('click', (e) => {
    e.preventDefault();
    store.del('gd_ref_key');
    location.href = '/indique';
  });

  const qk = new URLSearchParams(location.search).get('k');
  if (qk) {
    store.set('gd_ref_key', qk);
    history.replaceState(null, '', '/indique#painel');
  }
  const saved = qk || store.get('gd_ref_key');
  if (saved) loadPanel(saved, !!qk).catch(() => store.del('gd_ref_key'));

  // ---------------------------------------------------------------- entrada suave
  const io = new IntersectionObserver(
    (es) => es.forEach((x) => x.isIntersecting && (x.target.classList.add('in'), io.unobserve(x.target))),
    { threshold: 0.12 }
  );
  $$('.reveal').forEach((el) => io.observe(el));
})();
