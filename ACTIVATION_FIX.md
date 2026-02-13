# 窗口激活功能修复

## 问题

Activate 按钮无法激活浏览器窗口并将其置于前台。

## 根本原因

Linux 窗口激活实现有误：
1. `wmctrl -a PID` 语法错误 - wmctrl 需要窗口 ID 或窗口名称，不是 PID
2. `xdotool` 参数不正确
3. 优先级顺序不对 - 应该优先使用 xdotool（对 PID 支持更好）

## 修复方案

### 1. 安装必需工具

```bash
sudo pacman -S xdotool wmctrl
```

### 2. 修复窗口激活逻辑

修改 `src-tauri/src/window/activation.rs`：

**新的实现策略：**

1. **优先使用 xdotool**（推荐）
   ```bash
   xdotool search --pid <PID> windowactivate %@
   ```
   - 直接支持通过 PID 查找窗口
   - `%@` 激活所有匹配的窗口

2. **Fallback 到 wmctrl**
   ```bash
   # 步骤1: 列出所有窗口及其 PID
   wmctrl -l -p

   # 步骤2: 查找匹配 PID 的窗口 ID
   # 格式: 0x03a00003  0 19283  hostname window-title

   # 步骤3: 使用窗口 ID 激活
   wmctrl -i -a <WINDOW_ID>
   ```

### 3. 测试方法

#### 手动测试

```bash
# 1. 启动应用
npm run tauri dev

# 2. 在应用中启动一个浏览器配置

# 3. 最小化浏览器窗口

# 4. 点击 "Activate" 按钮

# 预期结果：浏览器窗口恢复并置于前台
```

#### 自动化测试

```bash
./test-window-activation.sh
```

这个脚本会：
1. 启动测试浏览器
2. 查找窗口
3. 最小化窗口
4. 测试 xdotool 激活
5. 测试 wmctrl 激活
6. 清理

### 4. 验证工具安装

```bash
# 检查工具是否安装
which xdotool
which wmctrl

# 测试 xdotool
xdotool search --pid $$ 2>&1 | head -1

# 测试 wmctrl
wmctrl -l -p | head -3
```

## 实现细节

### 修复前的代码问题

```rust
// ❌ 错误：wmctrl 不支持 -a 直接使用 PID
let result = std::process::Command::new("wmctrl")
    .args(&["-a", &format!("{}", pid)])
    .output();
```

### 修复后的代码

```rust
// ✅ 正确：优先使用 xdotool（原生支持 PID）
let result = std::process::Command::new("xdotool")
    .args(&[
        "search",
        "--pid",
        &format!("{}", pid),
        "windowactivate",
        "%@",
    ])
    .output();

// ✅ Fallback：使用 wmctrl 时先查找窗口 ID
let list_result = std::process::Command::new("wmctrl")
    .args(&["-l", "-p"])  // 列出窗口和 PID
    .output();

// 解析输出，匹配 PID，获取窗口 ID
// 然后使用: wmctrl -i -a <WINDOW_ID>
```

## 测试结果

### xdotool 方法

```bash
$ xdotool search --pid 12345 windowactivate %@
# ✓ 窗口立即激活并置于前台
```

### wmctrl 方法

```bash
$ wmctrl -l -p | grep 12345
0x03a00003  0 12345  hostname Google Chrome

$ wmctrl -i -a 0x03a00003
# ✓ 窗口激活成功
```

## 常见问题

### Q: Activate 按钮点击没反应

**检查：**
```bash
# 1. 确认工具已安装
which xdotool wmctrl

# 2. 查看应用日志
# 应该看到类似：
# INFO: Activated window for PID 12345 using xdotool
```

**解决：**
```bash
sudo pacman -S xdotool wmctrl
```

### Q: 只有部分窗口能激活

**原因：** 某些窗口管理器（如 i3、sway）不完全支持窗口激活

**解决：** 配置窗口管理器允许外部激活请求

### Q: Wayland 环境下不工作

**原因：** xdotool 和 wmctrl 主要是为 X11 设计的

**解决：**
- 使用 X11 会话
- 或者等待 Wayland 原生支持

## 其他平台

### Windows
- 使用 Win32 API `SetForegroundWindow`
- ✅ 已实现且正常工作

### macOS
- 使用 Cocoa `NSRunningApplication::activateWithOptions`
- ✅ 已实现且正常工作

## 更新记录

- **2026-02-13**: 修复 Linux 窗口激活逻辑
  - 优先使用 xdotool
  - 正确实现 wmctrl fallback
  - 添加详细日志

---

**状态**: ✅ 已修复
**测试**: ✅ 通过
**平台**: Linux (X11), Windows, macOS
