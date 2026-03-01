# 使用已存在浏览器会话指南

**功能：** 允许 AI Agent 连接到您手动启动并登录的浏览器

**版本：** v0.9.4+ (commit 20aa429+)

---

## 概述

这个功能允许您：
1. **手动启动浏览器** - 使用 Browsion UI 或命令行
2. **登录网站** - 在浏览器中进行人工操作、登录、配置
3. **Agent 接管控制** - 让 AI Agent 继续操作这个已登录的浏览器会话

---

## 使用场景

### 场景 1: 通过 UI 启动并登录

1. 在 Browsion UI 中创建一个 Profile（例如 `my-session`）
2. 启动浏览器
3. 手动登录到需要认证的网站（如 Gmail、GitHub、银行等）
4. Agent 使用 `launch_browser("my-session")` 连接到这个已运行的浏览器
5. Agent 可以操作已登录的会话

### 场景 2: 命令行启动 Chrome

1. 手动启动 Chrome 并开启 CDP：
   ```bash
   google-chrome \
     --user-data-dir=/tmp/my-chrome-session \
     --remote-debugging-port=9222 \
     --no-first-run \
     --no-default-browser-check
   ```

2. 在浏览器中登录并操作

3. 告诉 Browsion 这个浏览器：
   ```bash
   curl -X POST http://127.0.0.1:39525/api/register-external \
     -H "Content-Type: application/json" \
     -H "X-API-Key: your-api-key" \
     -d '{
       "profile_id": "my-chrome-session",
       "pid": 12345,
       "cdp_port": 9222
     }'
   ```

4. Agent 现在可以使用 `my-chrome-session` profile

### 场景 3: 通过 Browsion 启动后重新连接

1. Agent 使用 `launch_browser("my-profile")` 启动浏览器
2. Agent 执行一些操作
3. Agent 任务完成，浏览器保持运行
4. Tauri 应用重启或会话断开
5. Agent 再次调用 `launch_browser("my-profile")`
6. **自动连接到已运行的浏览器**（无需重新启动）

---

## API 端点

### 1. Launch Browser (支持连接已运行浏览器)

```http
POST /api/launch/:profile_id
```

**行为：**
- 如果浏览器**未运行**：启动新浏览器
- 如果浏览器**已在运行**：连接到现有会话（返回现有 PID 和 CDP 端口）

**响应：**
```json
{
  "pid": 12345,
  "cdp_port": 9222
}
```

### 2. Register External Browser

```http
POST /api/register-external
```

**请求体：**
```json
{
  "profile_id": "my-manual-browser",
  "pid": 12345,
  "cdp_port": 9222
}
```

**参数：**
- `profile_id` - 要注册的 profile ID（必须已存在）
- `pid` - Chrome 进程的 PID
- `cdp_port` - Chrome 的 remote debugging port

**响应：**
```json
{
  "pid": 12345,
  "cdp_port": 9222
}
```

**错误：**
- `404` - Profile 不存在
- `400` - CDP 端口不可访问
- `409` - Profile 已注册到不同的浏览器

### 3. Get Running Browsers

```http
GET /api/running
```

**响应：**
```json
[
  {
    "profile_id": "my-profile",
    "pid": 12345,
    "cdp_port": 9222,
    "launched_at": 1740832120
  }
]
```

---

## MCP 工具使用

### launch_browser

**描述：** 启动浏览器或连接到已运行的实例

```python
# 启动新浏览器（如果未运行）
launch_browser(profile_id="my-profile")

# 连接到已运行的浏览器
launch_browser(profile_id="my-profile")
```

**返回：**
```json
{
  "pid": 12345,
  "cdp_port": 9222
}
```

### register_external_browser

**描述：** 注册外部启动的浏览器

```python
# 注册手动启动的 Chrome
register_external_browser(
    profile_id="my-manual-browser",
    pid=12345,
    cdp_port=9222
)
```

**使用场景：**
- 您手动启动了 Chrome
- 您想告诉 Browsion 关于这个浏览器的信息
- 之后可以用 `launch_browser` 连接

### get_running_browsers

**描述：** 列出所有运行中的浏览器

```python
# 检查哪些 profile 正在运行
get_running_browsers()
```

---

## 完整工作流程示例

### 工作流 1: 手动登录 + 自动化操作

```python
# 1. 检查运行状态
get_running_browsers()

# 2. 连接到已运行的浏览器（或启动新的）
launch_browser(profile_id="my-session")
# 如果浏览器已在运行，会自动连接

# 3. 导航到网站（可能已登录）
navigate(profile_id="my-session", url="https://gmail.com")

# 4. 执行操作（利用已登录状态）
page_state = get_page_state(profile_id="my-session")
# 继续自动化操作...
```

### 工作流 2: 注册外部浏览器

```bash
# 1. 手动启动 Chrome
google-chrome \
  --user-data-dir=/tmp/my-work-session \
  --remote-debugging-port=9222

# 2. 在 Chrome 中登录、配置等...

# 3. 注册到 Browsion
curl -X POST http://127.0.0.1:39525/api/register-external \
  -H "Content-Type: application/json" \
  -H "X-API-Key: test-workflow-key" \
  -d '{
    "profile_id": "work-session",
    "pid": '"$(pgrep -f "remote-debugging-port=9222" | head -1)"',
    "cdp_port": 9222
  }'

# 4. Agent 现在可以使用
launch_browser(profile_id="work-session")
```

### 工作流 3: 跨会话保持浏览器运行

```python
# 会话 1: 启动浏览器
launch_browser(profile_id="persistent-session")
navigate(profile_id="persistent-session", url="https://example.com")
# ... 执行一些操作 ...

# 会话结束，但浏览器继续运行

# 会话 2: 重新连接到同一浏览器
launch_browser(profile_id="persistent-session")
# 自动连接，无需重新启动

# 继续之前的会话
get_page_state(profile_id="persistent-session")
```

---

## 重要说明

### CDP 端口要求

要使用这个功能，Chrome 必须使用 `--remote-debugging-port` 启动：

```bash
# 正确的启动方式
google-chrome --remote-debugging-port=9222

# 错误的启动方式（没有 CDP）
google-chrome  # ❌ 无法被 Browsion 控制
```

Browsion 启动的浏览器**自动包含** CDP 端口。

### Profile 必须存在

注册外部浏览器时，`profile_id` 必须已经存在于 Browsion 配置中：

```bash
# 首先创建 profile
curl -X POST http://127.0.0.1:39525/api/profiles \
  -H "Content-Type: application/json" \
  -H "X-API-Key: test-workflow-key" \
  -d '{
    "id": "my-session",
    "name": "My Session",
    "user_data_dir": "/tmp/my-session"
  }'

# 然后注册外部浏览器
curl -X POST http://127.0.0.1:39525/api/register-external ...
```

### 会话持久化

- Browsion 会保存浏览器会话信息到 `~/.browsion/running_sessions.json`
- Tauri 应用重启后，会自动探测并恢复运行中的浏览器
- 如果浏览器已关闭，会话信息会被自动清理

---

## 常见问题

### Q: 如何获取 Chrome 的 PID？

```bash
# 方法 1: pgrep
pgrep -f "remote-debugging-port=9222"

# 方法 2: ps
ps aux | grep "remote-debugging-port=9222"

# 方法 3: lsof
lsof -i :9222
```

### Q: 如何找到正确的 CDP 端口？

Browsion 使用以下端口范围：
- 默认从 9222 开始分配
- 每个浏览器使用不同的端口
- 使用 `get_running_browsers()` API 查看已分配的端口

### Q: 浏览器已运行但无法连接？

检查：
1. CDP 端口是否正确：`curl http://127.0.0.1:9222/json/version`
2. Profile 是否在 Browsion 中存在：`get_profiles()`
3. 是否使用了正确的 API Key

### Q: 可以注册其他浏览器（如 Firefox）吗？

**不可以**。目前只支持基于 Chromium 的浏览器（Chrome、Chromium、Edge 等），因为需要 CDP（Chrome DevTools Protocol）。

---

## 安全注意事项

1. **CDP 端口暴露** - CDP 端口允许完全控制浏览器
   - 默认只监听 `127.0.0.1`（本地）
   - 不要将其暴露到公网
   - 使用防火墙限制访问

2. **进程权限** - 注册外部浏览器需要知道 PID
   - 确保您拥有该进程
   - 恶意注册可能导致进程被误杀

3. **会话劫持** - 任何人能访问 API 的都可以控制浏览器
   - 使用 API Key 保护
   - 限制 API 访问权限
   - 在不可信环境中不要启用此功能

---

## 技术细节

### 会话恢复流程

1. **启动探测**
   - Browsion 启动时读取 `~/.browsion/running_sessions.json`
   - 对每个保存的会话探测 CDP 端口

2. **存活检查**
   - 发送 HTTP 请求到 `http://127.0.0.1:{cdp_port}/json/version`
   - 如果响应成功，浏览器仍在运行

3. **注册会话**
   - 调用 `process_manager.register_external()`
   - 恢复 ProcessInfo (PID, CDP port)

4. **清理死会话**
   - 如果浏览器不响应，移除会话记录
   - 每 30 秒自动清理死进程

### 连接池管理

```rust
// SessionManager 自动处理连接复用
pub async fn get_client(&self, profile_id: &str, cdp_port: u16) {
    // 1. 检查是否有现有连接
    if let Some(handle) = sessions.get(profile_id) {
        if handle.is_connected() {
            return handle; // 复用现有连接
        }
    }

    // 2. 创建新连接
    let client = CDPClient::attach(profile_id, cdp_port).await?;
    sessions.insert(profile_id, client);
}
```

---

*最后更新：* 2026-03-01
*相关功能：* Session Reconnect (F3), Proxy Presets (F6)
*相关提交：* [attach-to-existing-browser]
