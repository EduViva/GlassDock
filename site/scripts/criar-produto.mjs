// Cria o produto "GlassDock" (R$ 8,90) na AbacatePay. Rode uma vez:
//   ABACATEPAY_API_KEY=abc_... npm run produto
const key = process.env.ABACATEPAY_API_KEY;
if (!key) {
  console.error('Defina ABACATEPAY_API_KEY');
  process.exit(1);
}
const res = await fetch('https://api.abacatepay.com/v2/products/create', {
  method: 'POST',
  headers: { authorization: `Bearer ${key}`, 'content-type': 'application/json' },
  body: JSON.stringify({
    externalId: 'glassdock',
    name: 'GlassDock',
    description: 'Dock de vidro com widgets para Windows 10 e 11. Pagamento único',
    price: 890,
    currency: 'BRL',
  }),
});
const out = await res.json().catch(() => ({}));
if (!out.success) {
  console.error('Não deu certo:', out.error || res.status);
  process.exit(1);
}
console.log('Produto criado:', out.data.id, '(externalId glassdock)');
