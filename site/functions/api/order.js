import { abacate, json, markPaid } from '../../lib/core.js';

// GET /api/order?o=gd_xxx  →  { status, download? }
// Se o webhook ainda não chegou, pergunta direto para a AbacatePay. Assim a entrega
// funciona mesmo com o webhook atrasado, fora do ar ou mal configurado.
export async function onRequestGet({ request, env }) {
  const ext = new URL(request.url).searchParams.get('o') || '';
  if (!/^gd_[a-z0-9]{12}$/.test(ext)) return json({ error: 'Pedido inválido' }, 400);
  let o = await env.GD.get('order:' + ext, 'json');
  if (!o) return json({ error: 'Pedido não encontrado' }, 404);

  if (o.status !== 'paid' && o.checkoutId) {
    const r = await abacate(env, '/checkouts/get?id=' + encodeURIComponent(o.checkoutId));
    if (r.success && r.data) {
      const c = r.data;
      if (c.status === 'PAID') {
        let email = null;
        if (c.customerId) {
          const cu = await abacate(env, '/customers/get?id=' + encodeURIComponent(c.customerId));
          if (cu.success && cu.data) email = cu.data.email;
        }
        o = await markPaid(env, ext, {
          amount: c.paidAmount || c.amount,
          email,
          checkoutId: c.id,
          devMode: c.devMode,
          ref: o.ref,
        });
      } else if (['EXPIRED', 'CANCELLED', 'REFUNDED'].includes(c.status)) {
        return json({ status: c.status.toLowerCase() });
      }
    }
  }
  if (o.status === 'paid') return json({ status: 'paid', download: '/api/download?o=' + ext });
  return json({ status: o.status });
}
