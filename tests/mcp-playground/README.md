# Browsion MCP 测试环境

这是一个独立的测试和演示环境，用于使用 **Claude Code** 测试 Browsion MCP 服务器的所有功能。

## 功能特点

- ✅ **交互式测试环境** - 使用 Claude Code 直接与 MCP 服务器交互
- ✅ **完整功能覆盖** - 涵盖所有 73 个 MCP 工具，分为 12 个测试场景
- ✅ **代理支持** - 配置了代理服务器 `192.168.0.220:8889`
- ✅ **即用即试** - 一键启动，无需手动配置

## 快速开始

### 1. 启动 Browsion Tauri 应用（带 HTTP API）

```bash
cd /home/percy/works/browsion
npm run tauri dev
```

这将在 `http://127.0.0.1:38472` 启动 HTTP API。

### 2. 配置 Claude Code MCP 服务器

将 `mcp-config.json` 的内容添加到 Claude Code 的 MCP 配置中：

**Claude Code 配置位置：**
- Linux: `~/.config/claude/claude_desktop_config.json`
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`

在 `mcpServers` 中添加：

```json
{
  "browsion": {
    "command": "/home/percy/works/browsion/src-tauri/target/debug/browsion-mcp",
    "args": [],
    "env": {
      "BROWSION_API_PORT": "38472"
    }
  }
}
```

### 3. 重启 Claude Code

重启 Claude Code 以加载 MCP 服务器。

### 4. 创建测试 Profile

在 Tauri 应用中导入测试 profile：

1. 打开 Browsion 应用
2. 导入 `profiles/test-profile.json`
3. 启动 "MCP 测试 Profile"

### 5. 开始测试

现在你可以在 Claude Code 中使用 MCP 工具了！查看 `test-cases/` 目录中的测试场景。

## 测试场景

| 文件 | 测试内容 | MCP 工具数量 |
|------|----------|--------------|
| 01-profile-management.md | Profile CRUD 操作 | 5 |
| 02-browser-lifecycle.md | 启动/停止浏览器 | 3 |
| 03-navigation.md | 导航操作 | 7 |
| 04-mouse-keyboard.md | 鼠标键盘输入 | 6 |
| 05-forms-interaction.md | 表单交互 | 2 |
| 06-tabs-management.md | 标签页管理 | 4 |
| 07-cookies-storage.md | Cookie 和存储 | 8 |
| 08-screenshot.md | 截图功能 | 2 |
| 09-frames-dialogs.md | 框架和对话框 | 4 |
| 10-network-mocking.md | 网络拦截 | 3 |
| 11-emulation.md | 设备模拟 | 2 |
| 12-workflows-recording.md | 工作流和录制 | 5 |

## 代理配置

测试 profile 已配置使用代理服务器 `192.168.0.220:8889`，所有网络请求将通过此代理。

## 故障排查

**MCP 服务器未连接：**
- 确认 Browsion Tauri 应用正在运行
- 检查 HTTP API 端口 38472 是否可用
- 验证 MCP 配置文件路径正确

**Profile 启动失败：**
- 检查 Chrome 是否安装
- 确认用户数据目录权限
- 查看代理服务器 `192.168.0.220:8889` 是否可访问

## 目录结构

```
tests/mcp-playground/
├── README.md                    # 本文件
├── mcp-config.json              # Claude Code MCP 配置示例
├── profiles/
│   └── test-profile.json       # 测试用 profile（含代理配置）
├── test-cases/                 # 12 个测试场景文档
└── scripts/
    ├── setup.sh                # 环境设置脚本
    └── test-all.sh             # 批量测试脚本（可选）
```

## 技术细节

- **MCP 协议**: Model Context Protocol
- **传输方式**: stdio
- **API 版本**: Browsion HTTP API v1
- **默认端口**: 38472
- **Chrome 版本**: 支持 Stable/Beta/Dev/Canary
