# Browsion 快速参考

## 🚀 启动应用

```bash
cd /home/percy/works/browsion
npm run tauri dev
```

## 🎨 UI 布局

### 窗口尺寸
- 默认：1200 × 900
- 最小：900 × 700

### 网格布局（新！）
- **3 列显示**（1200px 宽度）
- **每列约 8 个 profile**
- **总共可见约 24 个 profile**

### 卡片尺寸
- 宽度：360px（最小）
- 高度：自适应
- 间距：0.75rem

## ⚙️ 核心功能

### 1. 添加 Profile
```
Profiles → [+ Add Profile]
→ 填写表单 → Save
```

### 2. 启动浏览器
```
找到 Profile → [Launch]
→ 浏览器启动 → 状态变为 "Running"
```

### 3. 激活窗口
```
Profile 运行中 → [Activate]
→ 浏览器窗口置于前台
```

### 4. 关闭浏览器
```
Profile 运行中 → [Kill]
→ 进程终止 → 状态变为 "Stopped"
```

### 5. 克隆配置（新！）
```
任意 Profile → [Clone]
→ 表单自动填充（名称 + " Copy"）
→ 修改需要的字段 → Save
→ 创建新 Profile
```

### 6. 编辑 Profile
```
Profile → [Edit]
→ 修改字段 → Save
```

### 7. 删除 Profile
```
Profile（未运行）→ [Delete]
→ 确认 → 删除
```

## 🔧 配置文件

### 位置
```
~/.config/browsion/config.toml
```

### 示例
```toml
chrome_path = "/path/to/chrome"

[[profiles]]
id = "uuid-xxx"
name = "US Profile"
description = "US proxy profile"
user_data_dir = "/home/user/profiles/us"
proxy_server = "http://192.168.0.220:8889"
lang = "en-US"
timezone = "America/Los_Angeles"
fingerprint = "1000"
color = "#4A90E2"
custom_args = []

[settings]
auto_start = false
minimize_to_tray = true
```

## 🎯 快捷操作

### 批量创建相似配置
1. 创建一个模板 Profile
2. 设置好通用参数（proxy、lang、timezone）
3. Clone 该 Profile
4. 只修改 `name`、`user_data_dir`、`fingerprint`
5. Save
6. 重复 3-5

### 快速切换浏览器
1. Launch 多个 Profile
2. 使用 [Activate] 在不同浏览器间切换
3. 无需手动查找窗口

## 🛠️ 故障排查

### Activate 不工作
```bash
# 安装必需工具
sudo pacman -S xdotool wmctrl
```

### Browse 按钮无响应
- 检查 `src-tauri/capabilities/default.json` 存在
- 确认包含 `"dialog:allow-open"` 权限

### 窗口显示问题
- 自动设置了 `WEBKIT_DISABLE_COMPOSITING_MODE=1`
- 如果仍有问题，检查图形驱动

## 📊 容量对比

| 窗口宽度 | 列数 | 每列行数 | 总可见 |
|---------|------|---------|--------|
| 900px   | 2    | ~8      | ~16    |
| 1200px  | 3    | ~8      | ~24    |
| 1600px+ | 4    | ~8      | ~32    |

## 🎨 按钮说明

| 按钮 | 颜色 | 功能 | 条件 |
|-----|------|------|------|
| Launch | 蓝色 | 启动浏览器 | 未运行时 |
| Activate | 绿色 | 激活窗口 | 运行中 |
| Kill | 红色 | 终止进程 | 运行中 |
| Edit | 灰色 | 编辑配置 | 任何时候 |
| Clone | 浅蓝色 | 克隆配置 | 任何时候 |
| Delete | 红色边框 | 删除配置 | 未运行时 |

## 📝 字段说明

### 必填字段
- **Name**: 配置名称
- **User Data Dir**: Chrome 用户数据目录

### 可选字段
- **Description**: 配置描述
- **Proxy Server**: 代理服务器（格式：`http://host:port`）
- **Language**: 语言代码（如 `en-US`）
- **Timezone**: 时区（如 `America/Los_Angeles`）
- **Fingerprint**: 指纹 ID
- **Color**: 颜色标签（用于视觉区分）
- **Custom Arguments**: 自定义启动参数（每行一个）

## 🔑 启动命令示例

实际执行的命令：
```bash
/path/to/chrome \
  --user-data-dir=/home/user/profiles/1000 \
  --fingerprint=1000 \
  --proxy-server=http://192.168.0.220:8889 \
  --lang=en-US \
  --timezone=America/Los_Angeles
```

## 💡 使用技巧

### 1. 颜色管理
- 按地区分类（US=蓝色，UK=红色）
- 按用途分类（测试=黄色，生产=绿色）

### 2. 命名规范
- 包含地区：`US-East-001`
- 包含用途：`Test-Profile-A`
- 有序编号：`Profile-1000`, `Profile-1001`

### 3. Clone 工作流
```
Template Profile
  ├─ Clone → Modify → Profile A
  ├─ Clone → Modify → Profile B
  └─ Clone → Modify → Profile C
```

### 4. 代理配置
- HTTP: `http://host:port`
- HTTPS: `https://host:port`
- SOCKS5: `socks5://host:port`

## 📚 相关文档

- `README.md` - 项目概述
- `FIXED_AND_TESTED.md` - 修复记录
- `ACTIVATION_FIX.md` - 窗口激活详解
- `UI_OPTIMIZATION.md` - UI 优化说明
- `GRID_LAYOUT_AND_CLONE.md` - 网格布局和克隆功能

---

**快速上手**: 1. 启动应用 → 2. 设置 Chrome 路径 → 3. 添加/Clone Profile → 4. Launch!
