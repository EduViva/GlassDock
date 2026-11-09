// Compra e indicação: usado em todas as páginas que têm botão de comprar.
(function () {
  const $ = (s, r = document) => r.querySelector(s);

  function toast(msg, ms = 3800) {
    const t = $('#toast');
    if (!t) return alert(msg);
    t.textContent = msg;
    t.classList.add('show');
    clearTimeout(t._h);
    t._h = setTimeout(() => t.classList.remove('show'), ms);
  }

  function cookie(name) {
    const m = document.cookie.match(new RegExp('(?:^|; )' + name + '=([^;]*)'));
    return m ? decodeURIComponent(m[1]) : null;
  }

  // quem indicou: vem do servidor (glassdock.com.br/nome) ou do cookie de uma visita anterior
  function currentRef() {
    const pill = $('[data-ref-pill][data-ref]');
    return (pill && pill.dataset.ref) || cookie('gd_ref') || null;
  }

  async function showRefFromCookie() {
    if ($('[data-ref-pill][data-ref]')) return;
    const slug = cookie('gd_ref');
    if (!slug || !/^[a-z0-9-]{1,24}$/.test(slug)) return;
    try {
      const r = await fetch('/api/ref?c=' + encodeURIComponent(slug));
      if (!r.ok) return;
      const { name } = await r.json();
      document.querySelectorAll('[data-ref-pill]').forEach((p) => {
        p.hidden = false;
        p.dataset.ref = slug;
      });
      document.querySelectorAll('[data-ref-name]').forEach((e) => (e.textContent = name));
      document.querySelectorAll('[data-ref-initial]').forEach((e) => (e.textContent = name.charAt(0).toUpperCase()));
    } catch {}
  }

  async function buy(btn) {
    if (btn) {
      btn.classList.add('loading');
      btn._html = btn.innerHTML;
      btn.innerHTML = '<span class="spinner"></span>Abrindo o pagamento';
    }
    try {
      const r = await fetch('/api/checkout', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ ref: currentRef() }),
      });
      const data = await r.json().catch(() => ({}));
      if (!r.ok || !data.url) throw new Error(data.error || 'Não deu para abrir o pagamento agora. Tente de novo em instantes');
      try {
        localStorage.setItem('gd_last_order', data.order);
      } catch {}
      location.href = data.url;
    } catch (e) {
      toast(e.message && !/fetch|JSON/i.test(e.message) ? e.message : 'Sem conexão com o servidor de pagamento agora. Tente de novo em instantes');
      if (btn) {
        btn.classList.remove('loading');
        btn.innerHTML = btn._html;
      }
    }
  }

  document.addEventListener('click', (e) => {
    const b = e.target.closest('[data-buy]');
    if (!b) return;
    e.preventDefault();
    buy(b);
  });

  window.GDBuy = { buy, toast, currentRef };
  showRefFromCookie();
})();
