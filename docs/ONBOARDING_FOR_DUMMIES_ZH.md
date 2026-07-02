# 为你的 API 接入 PR402 订阅支付 —— 新手指南

**简约至上，优雅为本。**

已有一个 Web API？想向用户收取按小时/按天/按月的访问费用？本指南手把手教你实现 —— **无需区块链知识**。

---

## 你将构建什么

**之前：**
```
你的 API → 任何人都可以免费调用
```

**之后：**
```
你的 API → 只有付费订阅者才能调用
           ↓
        支付一次（按小时/天/月）
           ↓
        获得 JWT 令牌
           ↓
        用令牌调用 API 直到过期
```

**用户体验：** 为时间窗口支付**一次**，在该时段内无限次使用你的 API。无需按请求付费。

---

## 两种方式：快速上手 vs. 生产部署

### 方式一：快速上手（5 分钟）
**适合：** 测试、单一服务、快速开始  
**原理：** 你的服务器在本地签发 JWT 令牌  
**配置：** 添加 3 个环境变量即可

### 方式二：生产部署（15 分钟）
**适合：** 多个服务、中心化管理、专业级部署  
**原理：** Miraland-labs 的集中式 JWT 签发服务（类似 Auth0）  
**配置：** 注册服务一次，配置卖家 SDK

**本指南涵盖两种方式。** 今天从方式一开始，准备好后切换到方式二。

---

## 前置条件

你需要：
- [ ] 一个现有的 Web API（任何语言：Node.js、Python、Go、Rust 等）
- [ ] 一个有 SOL 的 Solana 钱包（devnet 免费）
- [ ] 10 分钟时间

你**不**需要：
- ❌ 深入的区块链知识
- ❌ 智能合约经验
- ❌ 加密货币交易所账户（使用 devnet 测试）

---

## 方式一：快速上手（本地 JWT）

### 步骤 1：安装卖家 SDK

**Node.js/TypeScript：**
```bash
npm install @pr402/subscription-seller
```

**其他语言：** HTTP API（下面有示例）

### 步骤 2：设置环境变量

```bash
# 生成 JWT 密钥（任何 32+ 字符的字符串）
JWT_SECRET="your-secret-key-min-32-chars-long-here"

# 你的 Solana 钱包地址（接收付款的地址）
MERCHANT_WALLET="YourWalletAddressHere..."

# PR402 便捷服务商（测试用 devnet）
FACILITATOR_BASE_URL="https://preview.ipay.sh"

# 定价（示例：每小时 0.1 USDC）
X402_PAY_TO="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"  # USDC mint
X402_AMOUNT_HOURLY="100000"  # 0.1 USDC（6 位小数）
```

### 步骤 3：添加订阅端点

这是用户**付费**获取访问权限的地方：

**Node.js/Express：**
```javascript
const { createSubscriptionHandler } = require('@pr402/subscription-seller');

// 启动时配置一次
const subscriptionHandler = createSubscriptionHandler({
  jwtSecret: process.env.JWT_SECRET,
  merchantWallet: process.env.MERCHANT_WALLET,
  facilitatorBaseUrl: process.env.FACILITATOR_BASE_URL,
  tiers: {
    hourly: {
      mint: process.env.X402_PAY_TO,
      amount: process.env.X402_AMOUNT_HOURLY,
      duration: 3600  // 1 小时（秒）
    }
  }
});

// 订阅端点 - 用户 POST 到这里付费
app.post('/api/v1/subscribe', subscriptionHandler);
```

**其他语言（HTTP）：**
```
POST /api/v1/subscribe
Content-Type: application/json

{
  "tier": "hourly",
  "payer": "BuyerWalletAddress...",
  "paymentPayload": { /* pr402 付款证明 */ }
}

→ 向便捷服务商验证付款
→ 签发 JWT 令牌
→ 返回：{ "token": "eyJ...", "expiresAt": "2026-07-02T15:30:00Z" }
```

### 步骤 4：保护你现有的 API 路由

为 API 端点添加 JWT 验证：

**Node.js/Express：**
```javascript
const jwt = require('jsonwebtoken');

// 保护路由的中间件
function requireSubscription(req, res, next) {
  const token = req.headers.authorization?.replace('Bearer ', '');
  
  if (!token) {
    return res.status(401).json({ error: '未提供令牌' });
  }
  
  try {
    const decoded = jwt.verify(token, process.env.JWT_SECRET);
    
    // 检查是否过期
    if (Date.now() / 1000 >= decoded.exp) {
      return res.status(401).json({ error: '令牌已过期，请重新订阅' });
    }
    
    req.user = decoded;  // 将订阅者信息添加到请求
    next();
  } catch (err) {
    return res.status(401).json({ error: '无效令牌' });
  }
}

// 应用到你的 API 路由
app.get('/api/v1/data', requireSubscription, (req, res) => {
  // 你现有的 API 逻辑
  res.json({ data: '机密信息', subscriber: req.user.payer });
});
```

**Python/Flask：**
```python
import jwt
from functools import wraps

def require_subscription(f):
    @wraps(f)
    def decorated(*args, **kwargs):
        token = request.headers.get('Authorization', '').replace('Bearer ', '')
        
        if not token:
            return jsonify({'error': '未提供令牌'}), 401
        
        try:
            decoded = jwt.decode(token, os.getenv('JWT_SECRET'), algorithms=['HS256'])
            
            if time.time() >= decoded['exp']:
                return jsonify({'error': '令牌已过期，请重新订阅'}), 401
            
            request.user = decoded
            return f(*args, **kwargs)
        except:
            return jsonify({'error': '无效令牌'}), 401
    
    return decorated

@app.route('/api/v1/data')
@require_subscription
def get_data():
    return jsonify({'data': '机密信息', 'subscriber': request.user['payer']})
```

### 步骤 5：测试！

**终端 1 - 启动服务器：**
```bash
npm start  # 或 python app.py
```

**终端 2 - 测试订阅：**
```bash
# 安装测试客户端（或使用你自己的）
npx @pr402/subscription-client subscribe \
  --api-url http://localhost:3000 \
  --tier hourly \
  --keypair /path/to/buyer-wallet.json

# 调用受保护的 API
curl -H "Authorization: Bearer eyJ..." \
  http://localhost:3000/api/v1/data
```

**完成！** 你现在有了一个订阅门控的 API。🎉

---

## 方式二：生产部署（集中式认证服务）

为什么升级：
- ✅ **专业级：** RS256 令牌（类似 Auth0，非共享密钥）
- ✅ **零基础设施：** 使用 miraland-labs 托管的认证服务
- ✅ **可撤销：** 即时取消订阅
- ✅ **JWKS：** 标准令牌验证

**关键区别：** 不是自己部署，而是使用我们在 **https://auth.ipay.sh** 的集中式 JWT 签发服务

### 步骤 1：注册你的 API 服务

**每个要保护的 API 一次性配置：**

```bash
# 克隆仓库只是为了获取脚本
git clone https://github.com/miralandlabs/subscription-auth
cd subscription-auth/scripts
npm install

# 向 miraland-labs 的集中式服务注册
node register-service.mjs \
  --keypair /path/to/your-seller-wallet.json \
  --base-url https://auth.ipay.sh \
  --service-id api.myproduct.com \
  --service-url https://api.myproduct.com \
  --resources '["*"]'
```

**这做了什么：**
- 将 `api.myproduct.com` 注册为你在我们中心服务的服务 ID
- 链接到你的钱包（只有你可以为其签发令牌）
- 设置允许的资源（令牌可访问的路由）

**无需部署！** 我们为你运营认证服务。

### 步骤 2：配置你的 API 服务器

更新环境变量：

```bash
# 切换到 Tier B
SUBSCRIPTION_MODE=tier-b

# 指向 miraland-labs 的集中式认证服务
SUBSCRIPTION_AUTH_BASE_URL=https://auth.ipay.sh
SUBSCRIPTION_AUTH_ISS=https://auth.ipay.sh
SUBSCRIPTION_AUTH_SERVICE_ID=api.myproduct.com

# 你的钱包私钥（base58）- 用于签名挑战
SUBSCRIPTION_AUTH_MERCHANT_SECRET_KEY=your-base58-secret-key

# 撤销检查间隔
REVOCATION_POLL_INTERVAL_SEC=60

# 定价（与方式一相同）
FACILITATOR_BASE_URL=https://preview.ipay.sh
X402_PAY_TO=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
X402_AMOUNT_HOURLY=100000
```

### 步骤 3：更新订阅端点

**Node.js 使用 SDK：**
```javascript
const { 
  issueTokenViaAuthService,
  verifyTokenWithJwks 
} = require('@pr402/subscription-seller');

app.post('/api/v1/subscribe', async (req, res) => {
  const { tier, payer, paymentPayload } = req.body;
  
  // 1. 向便捷服务商验证付款（与方式一相同）
  const paymentVerified = await verifyPayment(paymentPayload);
  if (!paymentVerified) {
    return res.status(402).json({ error: '付款失败' });
  }
  
  // 2. 从 miraland-labs 的中心认证服务请求 RS256 令牌
  const result = await issueTokenViaAuthService({
    baseUrl: 'https://auth.ipay.sh',  // miraland-labs 的服务
    merchantWallet: process.env.MERCHANT_WALLET,
    serviceId: process.env.SUBSCRIPTION_AUTH_SERVICE_ID,
    payer,
    tier,
    resources: ['*'],
    signMessage: async (msg) => {
      // 用你的钱包签名挑战以证明所有权
      return signMessageWithWallet(msg, process.env.SUBSCRIPTION_AUTH_MERCHANT_SECRET_KEY);
    }
  });
  
  // 3. 将令牌返回给买家
  res.json({
    token: result.token,
    jti: result.jti,
    expiresAt: new Date(Date.now() + 3600000).toISOString(),
    persistenceHint: 'save-until-expire'
  });
});
```

### 步骤 4：更新 JWT 验证（JWKS）

**Node.js：**
```javascript
const jwksClient = require('jwks-rsa');
const jwt = require('jsonwebtoken');

// 创建 JWKS 客户端从 miraland-labs 的服务获取公钥
const client = jwksClient({
  jwksUri: 'https://auth.ipay.sh/.well-known/jwks.json',
  cache: true,
  cacheMaxAge: 600000  // 10 分钟
});

async function requireSubscription(req, res, next) {
  const token = req.headers.authorization?.replace('Bearer ', '');
  
  if (!token) {
    return res.status(401).json({ error: '未提供令牌' });
  }
  
  try {
    // 解码头部获取密钥 ID
    const decoded = jwt.decode(token, { complete: true });
    const key = await client.getSigningKey(decoded.header.kid);
    
    // 针对 miraland-labs 的公钥验证 RS256 签名
    const verified = jwt.verify(token, key.getPublicKey(), {
      issuer: 'https://auth.ipay.sh',
      algorithms: ['RS256']
    });
    
    // 检查撤销（轮询缓存每 60 秒更新）
    const revoked = await revocationCache.isRevoked(verified.jti);
    if (revoked) {
      return res.status(401).json({ error: '令牌已撤销' });
    }
    
    req.user = verified;
    next();
  } catch (err) {
    return res.status(401).json({ error: '无效令牌' });
  }
}
```

**完成！** 你现在使用 miraland-labs 的集中式服务拥有了生产级的订阅认证。🎉

---

## 测试你的配置

### 自动化测试（Tier B）

```bash
cd scripts
node e2e-tier-b-auth.mjs \
  --keypair /path/to/seller-wallet.json
```

### 手动测试流程

**1. 买家付费订阅：**
```bash
curl -X POST http://localhost:3000/api/v1/subscribe \
  -H "Content-Type: application/json" \
  -d '{
    "tier": "hourly",
    "payer": "BuyerWallet...",
    "paymentPayload": { /* pr402 付款 */ }
  }'

# 返回：
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "jti": "550e8400-e29b-41d4-a716-446655440000",
  "expiresAt": "2026-07-02T15:30:00Z",
  "tier": "hourly",
  "persistenceHint": "save-until-expire"
}
```

**2. 买家使用令牌调用 API：**
```bash
TOKEN="eyJhbGciOiJIUzI1NiIs..."

curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/v1/data

# 返回你的受保护数据
```

**3. 买家在令牌过期前不再付费**

---

## 常见模式

### 多层级定价

提供不同的定价层级：

```javascript
const tiers = {
  hourly: {
    mint: USDC_MINT,
    amount: 100000,      // 0.1 USDC
    duration: 3600       // 1 小时
  },
  daily: {
    mint: USDC_MINT,
    amount: 2000000,     // 2 USDC
    duration: 86400      // 24 小时
  },
  monthly: {
    mint: USDC_MINT,
    amount: 50000000,    // 50 USDC
    duration: 2592000    // 30 天
  }
};
```

用户选择层级：
```bash
POST /api/v1/subscribe
{ "tier": "daily", ... }
```

### 资源范围令牌

限制令牌可访问的端点：

```javascript
// 为特定资源签发令牌
await issueTokenViaAuthService({
  // ...
  resources: ['/api/v1/data', '/api/v1/reports']
});

// 在中间件中验证资源访问
function requireSubscription(req, res, next) {
  // ... 验证令牌 ...
  
  const requestPath = req.path;
  const allowedResources = req.user.resources;
  
  // 检查是否为通配符或允许特定路径
  if (!allowedResources.includes('*') && 
      !allowedResources.includes(requestPath)) {
    return res.status(403).json({ error: '不允许访问该资源' });
  }
  
  next();
}
```

### 优雅过期

帮助用户知道何时续费：

```javascript
app.get('/api/v1/data', requireSubscription, (req, res) => {
  const timeLeft = req.user.exp - Math.floor(Date.now() / 1000);
  
  res.json({
    data: '你的数据',
    subscription: {
      expiresIn: timeLeft,
      tier: req.user.tier,
      renewUrl: '/api/v1/subscribe'
    }
  });
});
```

### 提前撤销（仅 Tier B）

在订阅过期前取消：

```bash
# 卖家撤销令牌
node scripts/revoke-token.mjs \
  --keypair /path/to/seller-wallet.json \
  --jti 550e8400-e29b-41d4-a716-446655440000 \
  --service-id api.myproduct.com
```

你的 API 会自动检查撤销（60 秒轮询）。

---

## 故障排查

### "付款验证失败"

**原因：** PR402 便捷服务商无法验证付款  
**修复：**
- 检查 `FACILITATOR_BASE_URL` 是否正确
- 验证付款是否包含所有必需字段
- 先用 devnet USDC 测试

### "令牌已过期"

**原因：** 买家的令牌有效期已结束  
**修复：** 买家需要再次调用 `/api/v1/subscribe`（为新时段付费）

### "无效签名"（Tier B）

**原因：** JWT 签名与 JWKS 公钥不匹配  
**修复：**
- 验证 `SUBSCRIPTION_AUTH_ISS` 是否与你的部署匹配
- 检查 JWKS 端点是否可访问：`curl https://your-auth/.well-known/jwks.json`
- 确保 RSA 密钥对正确

### "浏览器 CORS 错误"

**原因：** 前端从不同域调用 API  
**修复：** 为你的 API 添加 CORS 头：
```javascript
app.use((req, res, next) => {
  res.header('Access-Control-Allow-Origin', '*');
  res.header('Access-Control-Allow-Headers', 'Authorization, Content-Type');
  next();
});
```

---

## 迁移：方式一 → 方式二

当你准备从快速上手升级到生产部署：

**1. 向 miraland-labs 的服务注册**（上面步骤 1）

**2. 更新环境变量：**
```bash
# 改变这个：
SUBSCRIPTION_MODE=tier-a
JWT_SECRET=...

# 改为这个：
SUBSCRIPTION_MODE=tier-b
SUBSCRIPTION_AUTH_BASE_URL=https://auth.ipay.sh
SUBSCRIPTION_AUTH_ISS=https://auth.ipay.sh
SUBSCRIPTION_AUTH_SERVICE_ID=api.myproduct.com
SUBSCRIPTION_AUTH_MERCHANT_SECRET_KEY=...
```

**3. 更新代码：** 将本地 JWT 签发替换为集中式认证服务调用（上面步骤 3）

**4. 部署：** 无需数据库迁移，旧令牌自然过期

**停机时间：** 零。两种模式使用相同的 `/api/v1/subscribe` 端点。

**注意：** 你不需要自己部署任何东西 - 只需向 miraland-labs 的集中式服务注册并调用它。

---

## 下一步

✅ **你已经门控了你的 API！** 用户现在按时间付费访问。

**接下来做什么：**

1. **添加分析：** 追踪订阅收入、热门层级
2. **添加 Webhook：** 订阅发生时通知你的系统
3. **添加试用期：** 首次付款前免费一小时
4. **添加团队计划：** 一次付款，多个用户共享令牌
5. **添加使用限制：** 每层级速率限制（每小时 = 100 请求/小时）

**资源：**

- [完整 API 参考](SUBSCRIPTION_AUTH_FOR_SELLERS.md)
- [线协议详情](PROTOCOL.md)
- [示例代码](https://github.com/miralandlabs/x402-subscription-starter)
- [买家 SDK](https://github.com/miralandlabs/x402-subscription-client)

**有问题？** 查看现有文档或提交 issue。

---

## 快速参考卡

```
┌─────────────────────────────────────────────────────┐
│ 订阅流程                                             │
├─────────────────────────────────────────────────────┤
│ 1. 买家：POST /api/v1/subscribe                     │
│    → 发送：{ tier, payer, paymentPayload }          │
│                                                     │
│ 2. 卖家：验证付款 → 签发 JWT                        │
│    → 返回：{ token, expiresAt }                     │
│                                                     │
│ 3. 买家：保存令牌，调用 API                         │
│    → 头部：Authorization: Bearer <token>           │
│                                                     │
│ 4. 卖家：每次 API 调用验证 JWT                      │
│    → 检查：签名、过期、撤销                         │
│                                                     │
│ 5. 过期时：买家重复步骤 1                           │
└─────────────────────────────────────────────────────┘

付款：每个时间窗口一次（小时/天/月）
API 调用：窗口期内无限次
令牌：Authorization 头中的 Bearer JWT
安全：RS256 或 HS256，标准 JWT 验证
```

**记住：** 简约至上，优雅为本。从简单开始（方式一），准备好后升级（方式二）。
