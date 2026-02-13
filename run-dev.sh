#!/bin/bash
# Browsion 开发模式启动脚本
# 使用软件渲染避免 GPU 问题

export WEBKIT_DISABLE_COMPOSITING_MODE=1

echo "🚀 Starting Browsion in development mode..."
echo "📝 Logs: Check terminal output"
echo "🌐 Frontend: http://localhost:5173"
echo "💡 Tip: Check system tray for Browsion icon"
echo ""

npm run tauri dev
