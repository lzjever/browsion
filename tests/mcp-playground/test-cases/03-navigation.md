# 导航功能测试

测试 Browsion MCP 的页面导航功能。

## 前置条件

- Browsion Tauri 应用正在运行
- Claude Code 已连接到 MCP 服务器
- test-profile 已导入并启动浏览器

## 测试用例

### 1. 导航到 URL

**在 Claude Code 中执行：**

```
请导航到 https://example.com，等待页面完全加载完成。
```

**预期结果：**
- 浏览器导航到目标 URL
- 等待 load 事件触发
- 返回导航成功确认

**验证点：**
- ✅ 当前 URL 变为 `https://example.com`
- ✅ 页面完全加载

---

### 2. 获取当前 URL

**在 Claude Code 中执行：**

```
请告诉我浏览器当前所在的 URL 和页面标题。
```

**预期结果：**
- 返回当前 URL
- 返回页面标题

**验证点：**
- ✅ URL 为 `https://example.com` 或 `https://example.com/`
- ✅ 标题为 `Example Domain`

---

### 3. 后退和前进

**在 Claude Code 中执行：**

```
请先导航到 https://www.google.com，然后再导航到 https://www.github.com，
最后执行后退操作，返回到 google.com。
```

**预期结果：**
- 两次导航都成功
- 后退操作返回到上一页

**验证点：**
- ✅ 最终 URL 为 `https://www.google.com`
- ✅ 浏览历史记录正确

---

### 4. 刷新页面

**在 Claude Code 中执行：**

```
请刷新当前页面。
```

**预期结果：**
- 页面重新加载
- 返回刷新确认

**验证点：**
- ✅ 页面内容重新加载
- ✅ 页面状态保持一致

---

### 5. 等待特定 URL

**在 Claude Code 中执行：**

```
请导航到 https://httpbin.org/delay/2，
然后等待 URL 包含 "/delay"。
```

**预期结果：**
- 导航成功
- 等待直到 URL 满足条件

**验证点：**
- ✅ 成功等待到目标 URL
- ✅ 超时机制正常工作

---

### 6. 等待页面元素

**在 Claude Code 中执行：**

```
请导航到 https://example.com，然后等待 id 为 "h1" 或其他主要元素出现。
```

**预期结果：**
- 导航到页面
- 等待指定元素出现在 DOM 中

**验证点：**
- ✅ 元素检测成功
- ✅ 返回元素信息

---

### 7. 等待特定文本

**在 Claude Code 中执行：**

```
请导航到 https://example.com，然后等待页面包含文本 "Example"。
```

**预期结果：**
- 导航成功
- 等待直到页面文本包含指定内容

**验证点：**
- ✅ 文本检测成功
- ✅ 超时机制正常

---

## 测试总结

本场景测试页面导航的核心功能：
- ✅ navigate - 导航到 URL
- ✅ get_current_url - 获取当前 URL
- ✅ get_page_title - 获取页面标题
- ✅ go_back - 后退
- ✅ go_forward - 前进
- ✅ reload - 刷新页面
- ✅ wait_for_url - 等待特定 URL
- ✅ wait_for_element - 等待元素出现
- ✅ wait_for_text - 等待文本出现

**代理验证：**
所有导航请求都应通过 `192.168.0.220:8889` 代理服务器。
