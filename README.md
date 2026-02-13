# Browsion 🌐

**跨平台浏览器启动器 - 轻松管理多个 Chrome 配置文件**

## ✨ 特性

- 🖥️ **跨平台**: 支持 Windows、macOS、Linux
- 🎯 **常驻托盘**: 系统托盘一键快速访问
- 📋 **配置管理**: 管理多个浏览器启动配置
- 🚀 **一键启动**: 快速启动预配置的浏览器实例
- 🔄 **进程追踪**: 实时监控浏览器运行状态
- 🪟 **窗口激活**: 快速切换到已启动的浏览器
- ⚙️ **灵活配置**: 支持代理、时区、语言、指纹等参数

## 🚀 快速开始

### 前置要求（Linux）

```bash
# 安装窗口管理工具（激活功能需要）
sudo pacman -S xdotool wmctrl  # Arch/Manjaro
sudo apt install xdotool wmctrl  # Ubuntu/Debian
```

### 运行开发模式

```bash
cd /home/percy/works/browsion

# 直接运行（环境变量已自动设置）
npm run tauri dev

# 或使用启动脚本
./run-dev.sh
```

### 构建生产版本

```bash
npm run tauri build
```

## 📖 使用指南

1. **启动应用**: 运行 `./run-dev.sh`
2. **配置 Chrome 路径**: Settings → 设置 Chrome 路径
3. **添加配置**: Profiles → Add Profile
4. **启动浏览器**: 点击 Launch 按钮
5. **管理窗口**: 使用 Activate/Kill 按钮

## ⚙️ 配置示例

配置文件: `~/.config/browsion/config.toml`

```toml
chrome_path = "/usr/bin/google-chrome"

[settings]
auto_start = false
minimize_to_tray = true

[[profiles]]
id = "uuid-1234"
name = "US Profile"
description = "美国代理配置"
user_data_dir = "/home/user/chrome_profiles/us"
proxy_server = "http://192.168.0.220:8889"
lang = "en-US"
timezone = "America/Los_Angeles"
fingerprint = "10000"
color = "#4A90E2"
custom_args = []
```

## 📚 文档

- [FIXED_AND_TESTED.md](FIXED_AND_TESTED.md) - 修复记录和测试指南
- [PROJECT_STATUS.md](PROJECT_STATUS.md) - 项目状态
- [TEST_GUIDE.md](TEST_GUIDE.md) - 详细测试指南

## 🐛 故障排除

### 应用无法启动
```bash
# 确保使用环境变量
export WEBKIT_DISABLE_COMPOSITING_MODE=1
./run-dev.sh
```

### 无法启动浏览器
在 Settings 中设置正确的 Chrome 路径:
- Linux: `/usr/bin/google-chrome`
- Windows: `C:\Program Files\Google\Chrome\Application\chrome.exe`
- macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`

### 窗口激活不工作 (Linux)
```bash
sudo pacman -S wmctrl xdotool  # Arch/Manjaro
sudo apt install wmctrl xdotool  # Ubuntu/Debian
```

## 🛠️ 技术栈

- **后端**: Rust + Tauri 2.0
- **前端**: React 18 + TypeScript
- **构建**: Vite 5
- **配置**: TOML

## 📝 项目结构

```
browsion/
├── src-tauri/          # Rust 后端
│   ├── src/config/     # 配置管理
│   ├── src/process/    # 进程管理
│   ├── src/window/     # 窗口激活
│   └── src/tray/       # 系统托盘
├── src/                # React 前端
│   ├── components/     # UI 组件
│   ├── api/           # API 封装
│   └── types/         # 类型定义
└── run-dev.sh         # 启动脚本
```

## 🎯 启动命令示例

```bash
/usr/bin/google-chrome \
  --user-data-dir=/home/user/chrome_profiles/us \
  --fingerprint=10000 \
  --proxy-server=http://192.168.0.220:8889 \
  --lang=en-US \
  --timezone=America/Los_Angeles
```

## 📄 许可

MIT License

---

**Made with ❤️ using Rust and Tauri**
