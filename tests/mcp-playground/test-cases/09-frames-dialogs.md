# 框架和对话框测试

测试 Browsion MCP 的 iframe 和 JavaScript 对话框处理功能。

## 前置条件

- Browsion Tauri 应用正在运行
- Claude Code 已连接到 MCP 服务器
- test-profile 已启动浏览器

## 测试用例

### 1. 获取框架信息

**在 Claude Code 中执行：**

```
请创建一个包含 iframe 的测试页面：
URL: data:text/html,<iframe src="https://example.com" id="myframe"></iframe>
然后列出所有框架。
```

**预期结果：**
- 返回框架列表
- 包含主框架和 iframe

**验证点：**
- ✅ 至少有两个框架（主页面 + iframe）
- ✅ 框架信息完整（id、url、parentId）

---

### 2. 切换到 iframe

**在 Claude Code 中执行：**

```
请创建一个带 iframe 的页面，
然后切换到 iframe 上下文，
获取 iframe 的标题。
```

**预期结果：**
- 切换到 iframe 上下文
- 后续操作在 iframe 中执行
- 返回 iframe 的标题（Example Domain）

**验证点：**
- ✅ 成功切换到 iframe
- ✅ 可以操作 iframe 内的元素

---

### 3. 返回主框架

**在 Claude Code 中执行：**

```
请从 iframe 切换回主框架（top level）。
```

**预期结果：**
- 切换回主文档
- 后续操作在主框架中执行

**验证点：**
- ✅ 成功切换回主框架
- ✅ 可以操作主文档元素

---

### 4. 处理 alert 对话框

**在 Claude Code 中执行：**

```
请创建一个包含 alert("Hello World") 的页面，
然后接受（accept）这个对话框。
```

**预期结果：**
- alert 对话框被接受
- 页面继续执行

**验证点：**
- ✅ 对话框被处理
- ✅ 页面不会挂起

---

### 5. 处理 confirm 对话框

**在 Claude Code 中执行：**

```
请创建一个包含 confirm("Are you sure?") 的页面，
然后取消（dismiss）这个对话框。
```

**预期结果：**
- confirm 对话框被取消
- 返回 false 结果

**验证点：**
- ✅ 对话框被正确处理
- ✅ 操作被取消

---

### 6. 处理 prompt 对话框

**在 Claude Code 中执行：**

```
请创建一个包含 prompt("Enter your name:") 的页面，
然后在提示框中输入 "Test User"。
```

**预期结果：**
- prompt 对话框被处理
- 输入值被传递给页面

**验证点：**
- ✅ 对话框处理成功
- ✅ 输入值正确

---

## 测试总结

本场景测试框架和对话框功能：
- ✅ get_frames - 获取所有框架
- ✅ switch_frame - 切换框架上下文
- ✅ main_frame - 返回主框架
- ✅ handle_dialog - 处理 JavaScript 对话框

**对话框类型：**
- alert - 警告对话框（只有确定按钮）
- confirm - 确认对话框（确定/取消）
- prompt - 输入对话框（文本输入）

**对话框操作：**
- accept - 接受（点击确定/输入值）
- dismiss - 取消（点击取消/关闭）

**框架注意事项：**
- 切换到 iframe 后，所有 DOM 操作都在 iframe 内
- 使用 main_frame() 返回主文档
- 某些网站可能禁止 iframe 嵌入（X-Frame-Options）
