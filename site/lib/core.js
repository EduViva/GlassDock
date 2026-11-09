// Código compartilhado pelas Functions do site (Cloudflare Pages).
//
// Dados no KV (binding GD):
//   ref:<slug>          indicador { slug, name, firstName, email, pixKey, pixType, key, createdAt }
//   key:<chave>         slug do indicador dono daquela chave de acesso ao painel
//   email:<email>       slug já cadastrado com esse e-mail
//   clicks:<slug>       visitas pelo link do indicador
//   order:<ext>         pedido { ext, ref, status, checkoutId, email, amount, downloads, ... }
//   com:<slug>:<ext>    comissão { ext, ref, amount, status, ... } (metadata repete o resumo)
//   file:GlassDock.exe  o executável entregue depois do pagamento

export const PRICE = 890; // centavos
export const COMMISSION_RATE = 0.5;
export const COMMISSION = Math.floor(PRICE * COMMISSION_RATE);
const API = 'https://api.abacatepay.com/v2';

export const SLUG_RE = /^[a-z0-9](?:[a-z0-9-]{0,22}[a-z0-9])?$/;
export const RESERVED = new Set([
  'api', 'admin', 'assets', 'indique', 'indicacao', 'indicar', 'obrigado', 'comprar', 'compra',
  'index', 'termos', 'privacidade', 'ajuda', 'suporte', 'download', 'downloads', 'painel', 'login',
  'conta', 'glassdock', 'www', 'app', 'blog', 'faq', 'contato', 'favicon', 'robots', 'sitemap',
]);

export function json(data, status = 200, headers = {}) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store', ...headers },
  });
}

export function rid(n = 12) {
  const abc = 'abcdefghijkmnpqrstuvwxyz23456789';
  return [...crypto.getRandomValues(new Uint8Array(n))].map((b) => abc[b % abc.length]).join('');
}

export function getCookie(request, name) {
  const raw = request.headers.get('cookie') || '';
  for (const part of raw.split(';')) {
    const [k, ...v] = part.trim().split('=');
    if (k === name) return decodeURIComponent(v.join('='));
  }
  return null;
}

export function slugify(s) {
  return (s || '')
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 24)
    .replace(/-+$/g, '');
}

export async function getRef(env, slug) {
  slug = (slug || '').toLowerCase();
  if (!SLUG_RE.test(slug) || RESERVED.has(slug)) return null;
  return env.GD.get('ref:' + slug, 'json');
}

export function brl(cents) {
  return (cents / 100).toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' });
}

// ---------------------------------------------------------------- AbacatePay

export async function abacate(env, path, { method = 'GET', body } = {}) {
  if (!env.ABACATEPAY_API_KEY) return { success: false, error: 'ABACATEPAY_API_KEY não configurada' };
  let res;
  try {
    res = await fetch((env.ABACATEPAY_API_URL || API) + path, {
      method,
      headers: {
        authorization: `Bearer ${env.ABACATEPAY_API_KEY}`,
        'content-type': 'application/json',
        accept: 'application/json',
      },
      body: body ? JSON.stringify(body) : undefined,
    });
  } catch (e) {
    return { success: false, error: 'rede: ' + e.message };
  }
  let out;
  try {
    out = await res.json();
  } catch {
    out = null;
  }
  if (!out || typeof out !== 'object') return { success: false, error: `HTTP ${res.status}` };
  if (!res.ok && out.success !== false) return { success: false, error: out.error || `HTTP ${res.status}` };
  return out;
}

// ---------------------------------------------------------------- Webhooks

const enc = new TextEncoder();
// Chave pública que a AbacatePay usa para assinar o formato antigo (X-Webhook-Signature).
const ABACATEPAY_SHARED_KEY =
  't9dXRhHHo3yDEj5pVDYz0frf7q6bMKyMRmxxCPIPp3RCplBfXRxqlC6ZpiWmOqj4L63qEaeUOtrCI8P0VMUgo6iIga2ri9ogaHFs0WIIywSMg0q7RmBfybe1E5XJcfC4IW3alNqym0tXoAKkzvfEjZxV6bE0oG2zJrNNYmUCKZyV0KZ3JS8Votf9EAWWYdiDkMkpbMdPggfh1EqHlVkMiTady6jOR3hyzGEHrIz2Ret0xHKMbiqkr9HS1JhNHDX9';

async function hmacB64(keyBytes, data) {
  const key = await crypto.subtle.importKey('raw', keyBytes, { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  const sig = new Uint8Array(await crypto.subtle.sign('HMAC', key, enc.encode(data)));
  let s = '';
  for (const b of sig) s += String.fromCharCode(b);
  return btoa(s);
}

function same(a, b) {
  if (typeof a !== 'string' || typeof b !== 'string' || a.length !== b.length) return false;
  let d = 0;
  for (let i = 0; i < a.length; i++) d |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return d === 0;
}

// Aceita os dois formatos que a AbacatePay envia:
//  - Standard Webhooks (webhook-id / webhook-timestamp / webhook-signature), endpoints novos
//  - antigo: ?webhookSecret= na URL + X-Webhook-Signature (HMAC com a chave pública)
export async function verifyWebhook(request, raw, secret) {
  if (!secret) return false;
  const h = request.headers;
  const sig = h.get('webhook-signature');
  if (sig) {
    const id = h.get('webhook-id') || '';
    const ts = h.get('webhook-timestamp') || '';
    if (!/^\d+$/.test(ts) || Math.abs(Date.now() / 1000 - Number(ts)) > 300) return false;
    const keyBytes = secret.startsWith('whsec_')
      ? Uint8Array.from(atob(secret.slice(6)), (c) => c.charCodeAt(0))
      : enc.encode(secret);
    const expected = await hmacB64(keyBytes, `${id}.${ts}.${raw}`);
    return sig.split(' ').some((part) => {
      const [v, s] = part.split(',');
      return v === 'v1' && same(s, expected);
    });
  }
  const legacy = h.get('x-webhook-signature');
  const qs = new URL(request.url).searchParams.get('webhookSecret');
  if (legacy && same(qs || '', secret)) {
    return same(legacy, await hmacB64(enc.encode(ABACATEPAY_SHARED_KEY), raw));
  }
  return false;
}

// ---------------------------------------------------------------- Pedidos e comissões

// Marca o pedido como pago e registra a comissão. Pode ser chamado pelo webhook
// e pela consulta da página de obrigado ao mesmo tempo, então tudo aqui é idempotente.
export async function markPaid(env, ext, info = {}) {
  const key = 'order:' + ext;
  let o = await env.GD.get(key, 'json');
  if (!o) o = { ext, ref: info.ref || null, status: 'pending', createdAt: Date.now() };
  if (o.status === 'paid') return o;
  o.status = 'paid';
  o.paidAt = Date.now();
  o.amount = info.amount || o.amount || PRICE;
  o.email = (info.email || o.email || '').toLowerCase() || null;
  o.checkoutId = info.checkoutId || o.checkoutId || null;
  o.devMode = !!info.devMode;
  o.downloads = o.downloads || 0;
  await env.GD.put(key, JSON.stringify(o));
  if (o.ref) await addCommission(env, o);
  return o;
}

async function addCommission(env, o) {
  const ref = await getRef(env, o.ref);
  if (!ref) return;
  // indicar a si mesmo não vale
  if (o.email && ref.email && o.email === ref.email.toLowerCase()) return;
  const ckey = `com:${o.ref}:${o.ext}`;
  if (await env.GD.get(ckey)) return;
  const c = {
    ext: o.ext,
    ref: o.ref,
    amount: Math.floor((o.amount || PRICE) * COMMISSION_RATE),
    status: 'pendente',
    devMode: o.devMode,
    createdAt: Date.now(),
  };
  await putCommission(env, ckey, c);
  if (env.AUTO_PAYOUT === '1') {
    try {
      await payCommission(env, ckey);
    } catch (e) {
      console.log('repasse automático falhou', ckey, e && e.message);
    }
  }
}

export function putCommission(env, ckey, c) {
  return env.GD.put(ckey, JSON.stringify(c), {
    metadata: { amount: c.amount, status: c.status, createdAt: c.createdAt, paidAt: c.paidAt || null },
  });
}

export async function cancelCommission(env, o, reason) {
  if (!o.ref) return;
  const ckey = `com:${o.ref}:${o.ext}`;
  const c = await env.GD.get(ckey, 'json');
  if (!c || c.status !== 'pendente') return;
  c.status = 'cancelada';
  c.reason = reason;
  await putCommission(env, ckey, c);
}

function pixKey(ref) {
  const k = ref.pixKey;
  if (ref.pixType === 'PHONE') return '+55' + k.replace(/\D/g, '').replace(/^55(?=\d{10,11}$)/, '');
  if (ref.pixType === 'CPF' || ref.pixType === 'CNPJ') return k.replace(/\D/g, '');
  return k.trim();
}

// Envia o PIX da comissão para o indicador. A AbacatePay desconta a tarifa do valor
// enviado; PAYOUT_FEE_CENTS soma essa tarifa para o indicador receber os 50% inteiros.
export async function payCommission(env, ckey) {
  const c = await env.GD.get(ckey, 'json');
  if (!c) return { ok: false, error: 'comissão não encontrada' };
  if (c.status !== 'pendente') return { ok: false, error: 'comissão já está ' + c.status };
  const ref = await getRef(env, c.ref);
  if (!ref) return { ok: false, error: 'indicador não encontrado' };
  c.status = 'pagando';
  await putCommission(env, ckey, c);
  const fee = parseInt(env.PAYOUT_FEE_CENTS || '0', 10) || 0;
  const r = await abacate(env, '/pix/send', {
    method: 'POST',
    body: {
      amount: c.amount + fee,
      externalId: 'com_' + c.ext,
      description: 'Indicação GlassDock',
      pix: { key: pixKey(ref), type: ref.pixType },
    },
  });
  if (!r.success) {
    c.status = 'pendente';
    c.lastError = r.error;
    await putCommission(env, ckey, c);
    return { ok: false, error: r.error };
  }
  c.status = 'paga';
  c.paidAt = Date.now();
  c.transferId = r.data.id;
  c.receiptUrl = r.data.receiptUrl;
  await putCommission(env, ckey, c);
  return { ok: true, commission: c };
}

export async function listAll(env, prefix) {
  const out = [];
  let cursor;
  do {
    const page = await env.GD.list({ prefix, cursor });
    out.push(...page.keys);
    cursor = page.list_complete ? undefined : page.cursor;
  } while (cursor);
  return out;
}
