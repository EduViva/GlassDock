import { cancelCommission, json, markPaid, verifyWebhook } from '../../lib/core.js';

// POST /api/webhook  (configure na AbacatePay com os eventos checkout.completed,
// checkout.refunded e checkout.disputed)
export async function onRequestPost({ request, env }) {
  const raw = await request.text();
  if (!(await verifyWebhook(request, raw, env.ABACATEPAY_WEBHOOK_SECRET))) {
    return json({ error: 'assinatura inválida' }, 401);
  }
  let ev;
  try {
    ev = JSON.parse(raw);
  } catch {
    return json({ error: 'json inválido' }, 400);
  }
  const type = ev.type || ev.event;
  const c = ev.data && ev.data.checkout;
  const ext = c && c.externalId;
  if (!ext || !/^gd_[a-z0-9]{12}$/.test(ext)) return json({ ok: true, ignored: true });

  if (type === 'checkout.completed' && c.status === 'PAID') {
    await markPaid(env, ext, {
      amount: c.paidAmount || c.amount,
      email: ev.data.customer && ev.data.customer.email,
      checkoutId: c.id,
      devMode: ev.devMode || c.devMode,
      ref: (c.metadata && c.metadata.ref) || null,
    });
  } else if (type === 'checkout.refunded' || type === 'checkout.disputed') {
    const o = await env.GD.get('order:' + ext, 'json');
    if (o) {
      o.status = type === 'checkout.refunded' ? 'refunded' : 'disputed';
      await env.GD.put('order:' + ext, JSON.stringify(o));
      await cancelCommission(env, o, type);
    }
  }
  return json({ ok: true });
}
