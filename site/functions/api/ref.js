import { COMMISSION, RESERVED, SLUG_RE, getRef, json, listAll, rid, slugify } from '../../lib/core.js';

const PIX_TYPES = ['CPF', 'CNPJ', 'EMAIL', 'PHONE', 'RANDOM'];

function validPix(type, key) {
  const d = key.replace(/\D/g, '');
  if (type === 'CPF') return d.length === 11;
  if (type === 'CNPJ') return d.length === 14;
  if (type === 'PHONE') return d.length >= 10 && d.length <= 13;
  if (type === 'EMAIL') return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(key);
  if (type === 'RANDOM') return /^[0-9a-f-]{32,36}$/i.test(key.trim());
  return false;
}

async function freeSlug(env, base) {
  if (!base) base = 'amigo';
  const tries = [base];
  for (let i = 2; i < 60; i++) tries.push(`${base}${i}`);
  for (const s of tries) {
    if (!SLUG_RE.test(s) || RESERVED.has(s)) continue;
    if (!(await env.GD.get('ref:' + s))) return s;
  }
  return base + rid(4);
}

// POST /api/ref  { name, email, pixType, pixKey, slug? }  →  cria o link de indicação
export async function onRequestPost({ request, env }) {
  let b = {};
  try {
    b = await request.json();
  } catch {}
  const name = String(b.name || '').trim().replace(/\s+/g, ' ').slice(0, 60);
  const email = String(b.email || '').trim().toLowerCase().slice(0, 120);
  const pixType = String(b.pixType || '').toUpperCase();
  const pixKey = String(b.pixKey || '').trim().slice(0, 120);
  if (name.length < 2) return json({ error: 'Escreva seu nome' }, 400);
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) return json({ error: 'E-mail inválido' }, 400);
  if (!PIX_TYPES.includes(pixType) || !validPix(pixType, pixKey)) {
    return json({ error: 'Confira a chave PIX' }, 400);
  }
  const taken = await env.GD.get('email:' + email);
  if (taken) {
    return json({ error: `Este e-mail já tem um link: /${taken}. Use a chave de acesso que você salvou` }, 409);
  }

  let slug;
  if (b.slug) {
    slug = slugify(b.slug);
    if (!SLUG_RE.test(slug) || RESERVED.has(slug)) return json({ error: 'Esse link não pode ser usado' }, 400);
    if (await env.GD.get('ref:' + slug)) return json({ error: 'Esse link já tem dono. Tente outro' }, 409);
  } else {
    slug = await freeSlug(env, slugify(name.split(' ')[0]));
  }

  const key = rid(20);
  const firstName = name.split(' ')[0];
  const ref = { slug, name, firstName, email, pixType, pixKey, key, createdAt: Date.now() };
  await env.GD.put('ref:' + slug, JSON.stringify(ref));
  await env.GD.put('key:' + key, slug);
  await env.GD.put('email:' + email, slug);
  return json({ slug, key, firstName });
}

function maskPix(t, k) {
  if (t === 'EMAIL') return k.replace(/^(.{2}).*(@.*)$/, '$1•••$2');
  const s = k.replace(/\s/g, '');
  return s.length > 6 ? s.slice(0, 3) + '•••' + s.slice(-3) : '•••';
}

// GET /api/ref?c=slug  →  nome público de quem indicou
// GET /api/ref?k=chave →  painel do indicador
export async function onRequestGet({ request, env }) {
  const q = new URL(request.url).searchParams;
  if (q.get('c')) {
    const ref = await getRef(env, q.get('c'));
    if (!ref) return json({ error: 'não encontrado' }, 404);
    return json({ slug: ref.slug, name: ref.firstName }, 200, { 'cache-control': 'public, max-age=300' });
  }
  const key = q.get('k') || '';
  if (!/^[a-z0-9]{20}$/.test(key)) return json({ error: 'Chave inválida' }, 400);
  const slug = await env.GD.get('key:' + key);
  const ref = slug && (await getRef(env, slug));
  if (!ref) return json({ error: 'Chave não encontrada' }, 404);

  const clicks = parseInt((await env.GD.get('clicks:' + slug)) || '0', 10);
  const keys = await listAll(env, `com:${slug}:`);
  const sum = { sales: 0, pending: 0, paid: 0 };
  const items = keys
    .map((k) => ({ ...(k.metadata || {}) }))
    .filter((m) => m.status && m.status !== 'cancelada')
    .sort((a, b) => b.createdAt - a.createdAt);
  for (const m of items) {
    sum.sales++;
    if (m.status === 'paga') sum.paid += m.amount;
    else sum.pending += m.amount;
  }
  return json({
    slug,
    name: ref.firstName,
    pix: { type: ref.pixType, key: maskPix(ref.pixType, ref.pixKey) },
    clicks,
    commission: COMMISSION,
    ...sum,
    items: items.slice(0, 50),
  });
}
