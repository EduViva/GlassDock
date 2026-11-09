import { abacate, getCookie, getRef, json, rid } from '../../lib/core.js';

// POST /api/checkout  { ref? }  →  { url }  (página de pagamento da AbacatePay)
export async function onRequestPost({ request, env }) {
  let body = {};
  try {
    body = await request.json();
  } catch {}
  const origin = new URL(request.url).origin;
  let ref = String(body.ref || getCookie(request, 'gd_ref') || '').toLowerCase();
  if (ref && !(await getRef(env, ref))) ref = null;

  const ext = 'gd_' + rid(12);
  const r = await abacate(env, '/checkouts/create', {
    method: 'POST',
    body: {
      items: [{ id: env.PRODUCT_ID || 'glassdock', quantity: 1 }],
      methods: (env.PAYMENT_METHODS || 'PIX,CARD').split(',').map((m) => m.trim()),
      externalId: ext,
      returnUrl: origin + '/',
      completionUrl: `${origin}/obrigado?o=${ext}`,
      metadata: { ref: ref || '', order: ext },
    },
  });
  if (!r.success) {
    console.log('checkout falhou', r.error);
    return json({ error: 'Não deu para abrir o pagamento agora. Tente de novo em instantes' }, 502);
  }
  const order = { ext, ref: ref || null, status: 'pending', checkoutId: r.data.id, createdAt: Date.now() };
  // pedidos não pagos somem sozinhos; ao pagar, o registro é regravado sem prazo
  await env.GD.put('order:' + ext, JSON.stringify(order), { expirationTtl: 60 * 60 * 24 * 7 });
  return json({ url: r.data.url, order: ext });
}
