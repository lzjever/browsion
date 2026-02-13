#!/bin/bash
# 测试窗口激活功能

echo "🧪 Testing window activation..."
echo ""

# 启动一个测试浏览器
echo "1. Launching test Chrome instance..."
/home/percy/tools/ungoogled-chromium-139.0.7258.154-1-x86_64_linux/chrome \
  --user-data-dir=/tmp/test-activation \
  --new-window \
  https://example.com &

CHROME_PID=$!
echo "   ✓ Chrome started with PID: $CHROME_PID"
sleep 3

# 等待窗口创建
echo ""
echo "2. Waiting for window to appear..."
sleep 2

# 使用 xdotool 查找窗口
WINDOW_ID=$(xdotool search --pid $CHROME_PID | head -1)
if [ -z "$WINDOW_ID" ]; then
    echo "   ✗ No window found for PID $CHROME_PID"
    kill $CHROME_PID 2>/dev/null
    exit 1
fi
echo "   ✓ Window found: $WINDOW_ID"

# 最小化窗口
echo ""
echo "3. Minimizing window..."
xdotool windowminimize $WINDOW_ID
sleep 1
echo "   ✓ Window minimized"

# 测试激活
echo ""
echo "4. Testing activation with xdotool..."
xdotool search --pid $CHROME_PID windowactivate
if [ $? -eq 0 ]; then
    echo "   ✓ xdotool activation successful"
else
    echo "   ✗ xdotool activation failed"
fi

sleep 2

# 测试 wmctrl 方法
echo ""
echo "5. Testing activation with wmctrl..."
wmctrl -i -a $WINDOW_ID
if [ $? -eq 0 ]; then
    echo "   ✓ wmctrl activation successful"
else
    echo "   ✗ wmctrl activation failed"
fi

echo ""
echo "✅ Test complete. Cleaning up..."
sleep 2
kill $CHROME_PID 2>/dev/null
rm -rf /tmp/test-activation

echo "Done!"
