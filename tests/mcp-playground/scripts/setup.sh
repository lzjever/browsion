# 设置脚本

这个脚本帮助快速设置 Browsion MCP 测试环境。

## 使用方法

```bash
cd /home/percy/works/browsion/tests/mcp-playground
chmod +x scripts/setup.sh
./scripts/setup.sh
```

## 脚本内容

```bash
#!/bin/bash
set -e

echo "======================================"
echo "Browsion MCP 测试环境设置"
echo "======================================"

# 1. 创建必要的目录
echo "📁 创建测试目录..."
mkdir -p ~/.browsion/mcp-test-profile

# 2. 编译 MCP 服务器（如果需要）
echo "🔨 检查 MCP 服务器编译状态..."
if [ ! -f "/home/percy/works/browsion/src-tauri/target/debug/browsion-mcp" ]; then
    echo "  ⚠️  MCP 服务器未编译，正在编译..."
    cd /home/percy/works/browsion/src-tauri
    cargo build --bin browsion-mcp
    echo "  ✅ 编译完成"
else
    echo "  ✅ MCP 服务器已存在"
fi

# 3. 检查代理服务器
echo "🌐 检查代理服务器..."
if nc -zv 192.168.0.220 8889 2>&1 | grep -q "succeeded"; then
    echo "  ✅ 代理服务器 192.168.0.220:8889 可访问"
else
    echo "  ⚠️  代理服务器 192.168.0.220:8889 不可访问"
    echo "     确认代理服务器正在运行"
fi

# 4. 检查 Chrome
echo "🌍 检查 Chrome 浏览器..."
if command -v google-chrome &> /dev/null; then
    echo "  ✅ 找到 google-chrome"
elif command -v chromium &> /dev/null; then
    echo "  ✅ 找到 chromium"
else
    echo "  ⚠️  未找到 Chrome，请安装"
fi

# 5. 显示 MCP 配置
echo ""
echo "======================================"
echo "📋 MCP 配置"
echo "======================================"
echo ""
echo "在 Claude Code 中添加以下 MCP 配置："
echo ""
echo "{"
echo "  \"browsion\": {"
echo "    \"command\": \"/home/percy/works/browsion/src-tauri/target/debug/browsion-mcp\","
echo "    \"args\": [],"
echo "    \"env\": {"
echo "      \"BROWSION_API_PORT\": \"38472\""
echo "    }"
echo "  }"
echo "}"
echo ""

# 6. 配置文件位置提示
echo "======================================"
echo "📝 配置文件位置"
echo "======================================"
echo ""
echo "Linux: ~/.config/claude/claude_desktop_config.json"
echo "macOS: ~/Library/Application Support/Claude/claude_desktop_config.json"
echo ""

# 7. 下一步提示
echo "======================================"
echo "🚀 下一步"
echo "======================================"
echo ""
echo "1. 启动 Browsion 应用："
echo "   cd /home/percy/works/browsion"
echo "   npm run tauri dev"
echo ""
echo "2. 在 Claude Code 的配置中添加 MCP 服务器（见上方配置）"
echo ""
echo "3. 重启 Claude Code"
echo ""
echo "4. 导入 test-profile.json 到 Browsion 应用"
echo ""
echo "5. 启动测试 profile 并开始测试"
echo ""
echo "======================================"
echo "✅ 设置完成！"
echo "======================================"
