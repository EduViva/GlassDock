import { getRef, json, listAll, payCommission, putCommission } from '../../lib/core.js';

function authorized(request, env) {
  const h = request.headers.get('authorization') || '';
  return !!env.ADMIN_KEY && env.ADMIN_KEY.length >= 16 && h === 'Bearer ' + env.ADMIN_KEY;
}

// GET /api/admin  →  comissões e vendas
export async function onRequestGet({ request, env }) {
  if (!authorized(request, env)) return json({ error: 'não autorizado' }, 401);
  const keys = await listAll(env, 'com:');
  const refs = {};
  const items = [];
  for (const k of keys) {
    const [, slug, ext] = k.name.split(':');
    if (!refs[slug]) {
      const r = await getRef(env, slug);
      refs[slug] = r ? { name: r.name, email: r.email, pixType: r.pixType, pixKey: r.pixKey } : null;
    }
    items.push({ id: k.name, slug, ext, ...(k.metadata || {}) });
  }
  items.sort((a, b) => b.createdAt - a.createdAt);
  const orders = (await listAll(env, 'order:')).length;
  const referrers = (await listAll(env, 'ref:')).length;
  return json({ items, refs, orders, referrers, autoPayout: env.AUTO_PAYOUT === '1' });
}

// POST /api/admin  { action: 'pay' | 'mark', id: 'com:slug:ext' }
export async function onRequestPost({ request, env }) {
  if (!authorized(request, env)) return json({ error: 'não autorizado' }, 401);
  const b = await request.json().catch(() => ({}));
  if (!/^com:[a-z0-9-]+:gd_[a-z0-9]{12}$/.test(b.id || '')) return json({ error: 'id inválido' }, 400);
  if (b.action === 'pay') {
    const r = await payCommission(env, b.id);
    return json(r, r.ok ? 200 : 400);
  }
  if (b.action === 'mark') {
    const c = await env.GD.get(b.id, 'json');
    if (!c) return json({ error: 'não encontrada' }, 404);
    c.status = 'paga';
    c.paidAt = Date.now();
    c.manual = true;
    await putCommission(env, b.id, c);
    return json({ ok: true });
  }
  return json({ error: 'ação inválida' }, 400);
}
