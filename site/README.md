# Site do GlassDock

Página de vendas (R$ 8,90, pagamento único) e programa de indicação (50% de cada venda), em Cloudflare Pages com Functions e KV. O pagamento passa pela AbacatePay.

```
public/          páginas e assets estáticos
  index.html     página de vendas
  indique.html   programa de indicação (cadastro + painel do indicador)
  obrigado.html  pós-pagamento: confirma e libera o download
  admin.html     comissões (pagar via PIX ou marcar como paga)
functions/       backend
  [code].js      glassdock.com.br/<nome>: página inicial com o nome de quem indicou
  api/checkout   cria a cobrança na AbacatePay
  api/order      status do pedido (consulta a AbacatePay se o webhook ainda não veio)
  api/webhook    recebe checkout.completed / refunded / disputed
  api/download   entrega o GlassDock.exe para pedidos pagos
  api/ref        cria link de indicação e mostra o painel
  api/admin      lista e paga comissões
lib/core.js      AbacatePay, assinatura de webhook, pedidos e comissões
```

## Como a compra funciona

1. O botão Comprar chama `/api/checkout`, que cria a cobrança (`POST /v2/checkouts/create`) com o produto `glassdock`, PIX e cartão, e manda a pessoa para a página de pagamento da AbacatePay.
2. Depois de pagar, a AbacatePay volta para `/obrigado?o=<pedido>`.
3. A página pergunta a `/api/order` se o pedido foi pago. Se o webhook já chegou, responde na hora. Se não chegou (atrasou, falhou ou nem foi configurado), a função consulta `GET /v2/checkouts/get` direto na AbacatePay. Por isso o download sai mesmo sem webhook.
4. O webhook continua importante para registrar a venda e a comissão quando a pessoa fecha a aba antes de voltar ao site.

## Como a indicação funciona

- Em `/indique` a pessoa informa nome, e-mail e chave PIX e recebe um link curto, por exemplo `glassdock.com.br/ana` (ou o nome que escolher).
- Quem abre o link vê a página normal com um selo pequeno "Ana te indicou o GlassDock". Um cookie guarda a indicação por 30 dias.
- Compra confirmada: 50% do valor pago (R$ 4,45) vira comissão. Compra com o mesmo e-mail do indicador não conta. Reembolso ou contestação cancela a comissão pendente.
- O indicador acompanha visitas, vendas e valores no próprio painel, com uma chave de acesso (sem senha).
- O PIX da comissão sai pela própria AbacatePay (`POST /v2/pix/send`): automático com `AUTO_PAYOUT=1`, ou com um clique em `/admin`.

## Colocar no ar

Precisa de Node 18+ e de uma conta grátis na Cloudflare.

```bash
cd site
npm install
npx wrangler login
```

**1. Banco (KV)**

```bash
npx wrangler kv namespace create GD
```

Copie o `id` mostrado para `wrangler.toml`, no lugar de `COLOQUE_O_ID_DO_KV_AQUI`.

**2. Projeto e segredos**

```bash
npx wrangler pages project create glassdock
npx wrangler pages secret put ABACATEPAY_API_KEY          # chave da API (comece com a de dev)
npx wrangler pages secret put ABACATEPAY_WEBHOOK_SECRET   # o segredo do webhook (passo 5)
npx wrangler pages secret put ADMIN_KEY                   # senha da página /admin, 16+ caracteres
```

**3. Produto na AbacatePay** (uma vez para dev e outra para produção, cada chave tem os seus produtos)

```bash
ABACATEPAY_API_KEY=abc_... npm run produto
```

Cria o produto `GlassDock` de R$ 8,90 com externalId `glassdock`. Se preferir criar pelo painel, use esse mesmo externalId (ou mude `PRODUCT_ID` no `wrangler.toml`).

**4. Publicar e enviar o executável**

```bash
npm run deploy
npm run subir-exe     # envia ../dist/GlassDock.exe para o KV
```

Rode `npm run subir-exe` de novo a cada versão nova do app. Quem já comprou baixa a versão nova pelo mesmo link. Se preferir hospedar o arquivo em outro lugar, defina `DOWNLOAD_URL` e o download vira um redirecionamento.

No painel da Cloudflare (Pages › glassdock › Custom domains) ligue o seu domínio.

**5. Webhook na AbacatePay**

No painel da AbacatePay, crie um webhook para `https://SEU-DOMINIO/api/webhook` com os eventos `checkout.completed`, `checkout.refunded` e `checkout.disputed`. Use o segredo cadastrado lá como `ABACATEPAY_WEBHOOK_SECRET`. A função aceita os dois formatos de assinatura da AbacatePay (Standard Webhooks e o antigo com `?webhookSecret=` na URL).

**6. Testar**

Com a chave de dev, compre pelo site, simule o pagamento no painel da AbacatePay e confira a página de obrigado, o download e a comissão em `/admin`. Depois troque `ABACATEPAY_API_KEY` pela chave de produção e crie o produto e o webhook de produção.

## Ajustes (`wrangler.toml`)

| Variável | Para que serve |
|---|---|
| `PRODUCT_ID` | externalId (ou id) do produto na AbacatePay |
| `PAYMENT_METHODS` | `PIX,CARD` (o cartão precisa estar liberado na sua conta) |
| `AUTO_PAYOUT` | `1` paga a comissão via PIX assim que a venda confirma; `0` deixa para o `/admin` |
| `PAYOUT_FEE_CENTS` | tarifa do PIX de saída, somada ao repasse para o indicador receber os R$ 4,45 inteiros |
| `MAX_DOWNLOADS` | quantos downloads cada pedido pode fazer |

O preço exibido nas páginas está escrito no HTML (R$ 8,90 / R$ 4,45). Se mudar o preço do produto, troque também `PRICE` em `lib/core.js` e os textos.

## Rodar local

```bash
cp .dev.vars.example .dev.vars   # coloque a chave de dev
npm run dev                      # http://localhost:8788
npx wrangler kv key put --binding=GD --local file:GlassDock.exe --path=../dist/GlassDock.exe
```
