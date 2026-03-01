# 网络拦截测试

测试 Browsion MCP 的网络请求拦截和 mock 功能。

## 前置条件

- Browsion Tauri 应用正在运行
- Claude Code 已连接到 MCP 服务器
- test-profile 已启动浏览器

## 测试用例

### 1. 阻止 URL 模式

**在 Claude Code 中执行：**

```
请阻止所有到 "*.example.com" 的请求，
然后尝试访问 https://example.com。
```

**预期结果：**
- 请求被阻止
- 返回阻止确认或错误

**验证点：**
- ✅ URL 模式阻止生效
- ✅ 无法访问被阻止的域名

---

### 2. Mock URL 响应

**在 Claude Code 中执行：**

```
请 mock 所有到 "*/api/*" 的请求，
返回 JSON 响应：{"status": "mocked", "message": "This is a mock response"}。
```

**预期结果：**
- API 请求被拦截
- 返回 mock 的响应

**验证点：**
- ✅ Mock 响应返回正确
- ✅ 原始 API 不会被调用

---

### 3. 获取网络日志

**在 Claude Code 中执行：**

```
请先清空网络日志，
然后访问几个页面，
最后告诉我所有的网络请求记录。
```

**预期结果：**
- 运行期间的网络请求被记录
- 包含请求 URL、方法、响应状态等

**验证点：**
- ✅ 网络日志完整
- ✅ 包含请求和响应信息

---

### 4. 清除网络拦截

**在 Claude Code 中执行：**

```
请清除所有 URL 阻止和 mock 规则，
恢复正常网络访问。
```

**预期结果：**
- 所有拦截规则清除
- 网络访问恢复正常

**验证点：**
- ✅ 可以正常访问之前被阻止的 URL

---

## 测试总结

本场景测试网络操作功能：
- ✅ block_url - 阻止 URL 模式
- ✅ mock_url - Mock URL 响应
- ✅ get_network_log - 获取网络日志
- ✅ clear_network_log - 清除网络日志
- ✅ clear_intercepts - 清除所有拦截规则

**拦截模式语法：**
- `*` - 通配符，匹配任意字符
- `*/api/*` - 匹配包含 /api/ 的路径
- `*.example.com` - 匹配 example.com 的任何子域名

**Mock 响应格式：**
- 必须是有效的 JSON
- 可以设置状态码（默认 200）
- 可以自定义响应头和内容

**与代理的关系：**
- 网络拦截在代理之后执行
- 即使用代理，拦截仍然生效
- 可以测试代理服务器的响应处理
