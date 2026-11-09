import { RESERVED, SLUG_RE, getCookie, getRef } from '../lib/core.js';

// glassdock.com.br/<nome>  →  a página inicial com o nome de quem indicou
export async function onRequestGet(context) {
  const { request, env, params, next } = context;
  const slug = String(params.code || '').toLowerCase();
  if (!SLUG_RE.test(slug) || RESERVED.has(slug)) return next();
  const ref = await getRef(env, slug);
  if (!ref) return next();

  if (getCookie(request, 'gd_ref') !== slug) {
    const n = parseInt((await env.GD.get('clicks:' + slug)) || '0', 10);
    context.waitUntil(env.GD.put('clicks:' + slug, String(n + 1)));
  }

  const page = await env.ASSETS.fetch(new URL('/', request.url));
  const name = ref.firstName;
  const res = new HTMLRewriter()
    .on('[data-ref-pill]', {
      element(el) {
        el.removeAttribute('hidden');
        el.setAttribute('data-ref', slug);
      },
    })
    .on('[data-ref-name]', {
      element(el) {
        el.setInnerContent(name);
      },
    })
    .on('[data-ref-initial]', {
      element(el) {
        el.setInnerContent(name.charAt(0).toUpperCase());
      },
    })
    .transform(page);

  const out = new Response(res.body, res);
  out.headers.set('cache-control', 'private, no-store');
  out.headers.append('set-cookie', `gd_ref=${slug}; Path=/; Max-Age=${60 * 60 * 24 * 30}; SameSite=Lax; Secure`);
  return out;
}
