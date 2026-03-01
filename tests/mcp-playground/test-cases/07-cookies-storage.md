# Cookie 和存储测试

测试 Browsion MCP 的 Cookie 和 Web Storage 管理功能。

## 前置条件

- Browsion Tauri 应用正在运行
- Claude Code 已连接到 MCP 服务器
- test-profile 已启动浏览器

## 测试用例

### 1. 设置 Cookie

**在 Claude Code 中执行：**

```
请在 https://example.com 上设置一个 Cookie：
- 名称: session_id
- 值: abc123xyz
- 域: .example.com
- 路径: /
```

**预期结果：**
- Cookie 设置成功
- 返回确认信息

**验证点：**
- ✅ Cookie 存储到浏览器
- ✅ 可以在后续请求中看到

---

### 2. 获取所有 Cookies

**在 Claude Code 中执行：**

```
请列出当前页面的所有 Cookies。
```

**预期结果：**
- 返回 Cookie 列表
- 包含名称、值、域、路径等信息

**验证点：**
- ✅ 包含刚设置的 session_id
- ✅ Cookie 信息完整

---

### 3. 删除 Cookie

**在 Claude Code 中执行：**

```
请删除名为 session_id 的 Cookie。
```

**预期结果：**
- 指定 Cookie 被删除
- 返回确认

**验证点：**
- ✅ Cookie 被移除
- ✅ 再次获取列表时不存在

---

### 4. 导出 Cookies

**在 Claude Code 中执行：**

```
请导出当前页面的所有 Cookies，
格式为 JSON。
```

**预期结果：**
- 返回 JSON 格式的 Cookies
- 可以保存到文件

**验证点：**
- ✅ JSON 格式正确
- ✅ 包含所有 Cookie 数据

---

### 5. 导入 Cookies

**在 Claude Code 中执行：**

```
请导入以下 Cookies：
[
  {"name": "test", "value": "value1", "domain": ".example.com", "path": "/"}
]
```

**预期结果：**
- Cookies 导入成功
- 可以在页面中使用

**验证点：**
- ✅ 导入的 Cookie 可用
- ✅ 获取 Cookie 列表时能看到

---

### 6. localStorage 操作

**在 Claude Code 中执行：**

```
请设置 localStorage 键值对：
- 键: username
- 值: testuser
然后读取它，
最后删除它。
```

**预期结果：**
- 设置成功
- 读取正确
- 删除成功

**验证点：**
- ✅ localStorage 操作正常
- ✅ 值正确存储和检索

---

### 7. sessionStorage 操作

**在 Claude Code 中执行：**

```
请设置 sessionStorage 项：
- 键: temp_data
- 值: temporary
然后读取并验证。
```

**预期结果：**
- sessionStorage 操作成功
- 数据可以读取

**验证点：**
- ✅ sessionStorage 正常工作
- ✅ 数据正确存储

---

### 8. 清空存储

**在 Claude Code 中执行：**

```
请清空所有 localStorage 数据。
```

**预期结果：**
- localStorage 被清空
- 返回确认

**验证点：**
- ✅ 所有数据被清除
- ✅ 存储为空

---

## 测试总结

本场景测试 Cookie 和存储功能：
- ✅ set_cookie - 设置 Cookie
- ✅ get_cookies - 获取所有 Cookies
- ✅ delete_cookies - 删除 Cookie
- ✅ export_cookies - 导出 Cookies
- ✅ import_cookies - 导入 Cookies
- ✅ set_storage - 设置存储项
- ✅ get_storage - 获取存储内容
- ✅ clear_storage - 清空存储

**存储类型：**
- localStorage - 持久化存储（除非手动清除）
- sessionStorage - 会话级存储（标签关闭时清除）

**Cookie 属性：**
- name - Cookie 名称
- value - Cookie 值
- domain - Cookie 域
- path - Cookie 路径
- expires - 过期时间（可选）
- httpOnly - HTTP only 标志
- secure - 安全标志
- sameSite - SameSite 属性

**代理影响：**
- Cookie 通过代理服务器传输
- 某些网站可能会设置额外的代理相关 Cookie
