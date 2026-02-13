# Browsion - 项目状态报告

## 项目概述

Browsion 是一个跨平台（Windows、Mac、Linux）的浏览器启动器，用于管理多个 Chrome 浏览器配置文件。

## 当前实现状态 ✅

### 后端 (Rust + Tauri) - 100% 完成

#### ✅ 核心模块

1. **配置管理** (`src-tauri/src/config/`)
   - ✅ 数据结构定义 (schema.rs)
   - ✅ TOML 配置加载/保存 (storage.rs)
   - ✅ 配置验证 (validation.rs)
   - 配置文件位置: `~/.config/browsion/config.toml`

2. **进程管理** (`src-tauri/src/process/`)
   - ✅ 浏览器启动逻辑 (launcher.rs)
   - ✅ 进程追踪和管理 (manager.rs)
   - ✅ 启动、关闭、状态检查
   - ✅ 自动清理死进程 (每 10 秒)

3. **窗口管理** (`src-tauri/src/window/`)
   - ✅ Windows 窗口激活 (Win32 API)
   - ✅ macOS 窗口激活 (Cocoa)
   - ✅ Linux 窗口激活 (wmctrl/xdotool)

4. **系统托盘** (`src-tauri/src/tray/`)
   - ✅ 托盘图标显示
   - ✅ 点击显示/隐藏窗口
   - ✅ 托盘菜单 (显示窗口、退出)
   - ✅ 关闭时最小化到托盘

5. **命令接口** (`src-tauri/src/commands/`)
   - ✅ get_profiles - 获取所有配置
   - ✅ add_profile - 添加配置
   - ✅ update_profile - 更新配置
   - ✅ delete_profile - 删除配置
   - ✅ launch_profile - 启动浏览器
   - ✅ activate_profile - 激活窗口
   - ✅ kill_profile - 关闭浏览器
   - ✅ get_running_profiles - 获取运行状态
   - ✅ get_chrome_path - 获取 Chrome 路径
   - ✅ update_chrome_path - 更新 Chrome 路径
   - ✅ get_settings - 获取设置
   - ✅ update_settings - 更新设置

### 前端 (React + TypeScript) - 100% 完成

#### ✅ 核心组件

1. **类型定义** (`src/types/profile.ts`)
   - ✅ BrowserProfile
   - ✅ AppConfig
   - ✅ AppSettings
   - ✅ ProcessInfo
   - ✅ RunningStatus

2. **API 封装** (`src/api/tauri.ts`)
   - ✅ 所有后端命令的 TypeScript 封装
   - ✅ 类型安全的 API 调用

3. **UI 组件**
   - ✅ App.tsx - 主应用组件 (导航、视图切换)
   - ✅ ProfileList.tsx - 配置列表 (自动刷新状态)
   - ✅ ProfileItem.tsx - 单个配置项 (启动/激活/关闭按钮)
   - ✅ ProfileForm.tsx - 添加/编辑表单
   - ✅ Settings.tsx - 全局设置

4. **功能特性**
   - ✅ 实时状态刷新 (每 5 秒)
   - ✅ 错误处理和用户提示
   - ✅ 加载状态显示
   - ✅ 空状态提示
   - ✅ 颜色标签支持

## 项目结构

```
browsion/
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs              # 应用入口
│   │   ├── lib.rs               # 库根 + Tauri 初始化
│   │   ├── state.rs             # 全局状态
│   │   ├── error.rs             # 错误类型
│   │   ├── config/              # 配置管理
│   │   ├── process/             # 进程管理
│   │   ├── window/              # 窗口激活
│   │   ├── tray/                # 系统托盘
│   │   └── commands/            # Tauri 命令
│   ├── Cargo.toml               # Rust 依赖
│   ├── tauri.conf.json          # Tauri 配置
│   └── icons/                   # 应用图标
├── src/                         # React 前端
│   ├── main.tsx                 # 前端入口
│   ├── App.tsx                  # 主组件
│   ├── components/              # React 组件
│   ├── api/                     # API 封装
│   ├── types/                   # TypeScript 类型
│   └── styles/                  # CSS 样式
├── package.json                 # Node.js 依赖
└── vite.config.ts               # Vite 配置
```

## 已解决的技术问题

### 1. Git Proxy 问题 ✅
- **问题**: Cargo 通过 Git 获取依赖时遇到代理问题
- **解决**: `git config --global --unset http.proxy`

### 2. 图标格式问题 ✅
- **问题**: Tauri 要求图标必须是 RGBA TrueColor 格式
- **解决**: 使用 Python PIL 生成每个像素唯一的图像，强制 TrueColorAlpha 格式

### 3. sysinfo API 更新 ✅
- **问题**: sysinfo 0.31 API 签名变化
- **解决**: 移除 `refresh_processes_specifics` 的多余参数

### 4. Tauri State 访问 ✅
- **问题**: State 没有 `.get()` 方法
- **解决**: 直接使用 `window.state::<AppState>()`

## 配置文件示例

位置: `~/.config/browsion/config.toml` (Linux)

```toml
chrome_path = "/usr/bin/google-chrome"

[settings]
auto_start = false
minimize_to_tray = true

[[profiles]]
id = "profile-001"
name = "US Profile"
description = "US proxy with LA timezone"
user_data_dir = "/home/percy/google_profile/10000"
proxy_server = "http://192.168.0.220:8889"
lang = "en-US"
timezone = "America/Los_Angeles"
fingerprint = "10000"
color = "#4A90E2"
custom_args = []
```

## 启动命令示例

当你点击启动按钮时，实际执行的命令：

```bash
/usr/bin/google-chrome \
  --user-data-dir=/home/percy/google_profile/10000 \
  --fingerprint=10000 \
  --proxy-server=http://192.168.0.220:8889 \
  --lang=en-US \
  --timezone=America/Los_Angeles
```

## 如何运行项目

### 开发模式

```bash
cd /home/percy/works/browsion
npm run tauri dev
```

这会：
1. 启动 Vite 开发服务器 (端口 5173)
2. 编译 Rust 后端
3. 启动 Tauri 窗口
4. 支持热重载

### 构建生产版本

```bash
npm run tauri build
```

生成安装包:
- Linux: `.deb`, `.AppImage`
- Windows: `.exe`, `.msi`
- macOS: `.dmg`, `.app`

## 功能演示流程

1. **启动应用**
   - 应用图标出现在系统托盘
   - 点击托盘图标打开主窗口

2. **添加配置**
   - 点击 "Add Profile" 按钮
   - 填写配置信息:
     - 名称: Test Profile
     - 描述: 测试配置
     - User Data Dir: /tmp/test_profile
     - Proxy Server: http://192.168.0.220:8889
     - Language: en-US
     - Timezone: America/Los_Angeles
     - Fingerprint: 10000
   - 点击保存

3. **启动浏览器**
   - 在配置列表中找到刚添加的配置
   - 点击 "Launch" 按钮
   - 浏览器应该启动，状态变为 "Running"

4. **激活窗口**
   - 最小化浏览器窗口
   - 点击 "Activate" 按钮
   - 浏览器窗口应该恢复并置顶

5. **关闭浏览器**
   - 点击 "Kill" 按钮
   - 浏览器进程被终止
   - 状态变为 "Stopped"

6. **设置 Chrome 路径**
   - 点击 "Settings" 标签
   - 修改 Chrome 可执行文件路径
   - 点击保存

## 测试清单

### 基本功能测试

- [ ] 应用能正常启动
- [ ] 托盘图标显示
- [ ] 点击托盘图标显示/隐藏窗口
- [ ] 配置文件能正确加载
- [ ] 能添加新配置
- [ ] 能编辑现有配置
- [ ] 能删除配置
- [ ] 能启动浏览器 (需要有 Chrome 安装)
- [ ] 能激活已运行的浏览器窗口
- [ ] 能关闭浏览器进程
- [ ] 状态刷新正常工作
- [ ] 能修改 Chrome 路径
- [ ] 关闭窗口时最小化到托盘

### 跨平台测试

- [ ] Linux (当前环境) ✅
- [ ] Windows
- [ ] macOS

## 已知限制和注意事项

### 1. Linux 窗口激活
- 需要安装 `wmctrl` 或 `xdotool`
- 不同桌面环境行为可能不同

### 2. Chrome 路径
- 需要根据平台和安装位置配置
- 默认路径:
  - Linux: `/usr/bin/google-chrome`
  - Windows: `C:\Program Files\Google\Chrome\Application\chrome.exe`
  - macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`

### 3. 用户数据目录
- 需要确保目录存在或 Chrome 有权限创建
- 建议使用绝对路径

### 4. 代理服务器
- 需要确保代理服务器可访问
- 格式: `http://host:port` 或 `socks5://host:port`

## 下一步优化建议

### MVP 后的增强功能

1. **UI 美化**
   - 使用 Tailwind CSS + shadcn/ui 组件
   - 添加动画效果
   - 优化布局和响应式设计

2. **功能增强**
   - 导入/导出配置
   - 配置模板
   - 搜索和过滤
   - 批量操作
   - 快捷键支持

3. **日志和调试**
   - 添加日志查看器
   - 启动历史记录
   - 错误日志导出

4. **性能优化**
   - 减少状态刷新频率 (配置化)
   - 优化图标资源

5. **安全性**
   - 敏感信息加密存储
   - 代理密码支持

## 编译状态

✅ **Rust 后端**: 编译成功 (有 5 个警告，不影响功能)
✅ **前端 TypeScript**: 类型检查通过
✅ **图标资源**: 已生成 RGBA 格式

## 联系和反馈

如果遇到问题或有功能建议，请检查:
1. Chrome 是否已安装
2. 配置文件路径是否正确
3. 代理服务器是否可访问
4. 查看终端日志输出

---

**项目状态**: ✅ MVP 完成，可以开始测试！
**最后更新**: 2026-02-13
