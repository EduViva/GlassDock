import { json } from '../../lib/core.js';

// GET /api/download?o=gd_xxx  →  GlassDock.exe (só para pedidos pagos)
export async function onRequestGet({ request, env }) {
  const ext = new URL(request.url).searchParams.get('o') || '';
  if (!/^gd_[a-z0-9]{12}$/.test(ext)) return json({ error: 'Pedido inválido' }, 400);
  const o = await env.GD.get('order:' + ext, 'json');
  if (!o || o.status !== 'paid') return json({ error: 'Pagamento ainda não confirmado' }, 403);

  const max = parseInt(env.MAX_DOWNLOADS || '15', 10);
  if ((o.downloads || 0) >= max) {
    return json({ error: 'Limite de downloads deste pedido atingido. Fale com o suporte' }, 429);
  }
  o.downloads = (o.downloads || 0) + 1;
  await env.GD.put('order:' + ext, JSON.stringify(o));

  if (env.DOWNLOAD_URL) return Response.redirect(env.DOWNLOAD_URL, 302);
  const file = await env.GD.get('file:GlassDock.exe', 'arrayBuffer');
  if (!file) return json({ error: 'Arquivo indisponível no momento. Fale com o suporte' }, 503);
  return new Response(file, {
    headers: {
      'content-type': 'application/vnd.microsoft.portable-executable',
      'content-disposition': 'attachment; filename="GlassDock.exe"',
      'cache-control': 'private, no-store',
    },
  });
}
