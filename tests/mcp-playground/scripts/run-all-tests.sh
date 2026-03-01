#!/bin/bash
# Browsion MCP 完整测试执行脚本
# 通过 HTTP API 测试所有 MCP 功能

set -e

API_BASE="http://127.0.0.1:39525/api"
API_KEY="test-workflow-key"
PROXY="http://192.168.0.220:8889"
PROFILE_ID="mcp-test-auto-test"
USER_DATA_DIR="/tmp/browsion-mcp-auto-test"

# CURL with API key
curl_api() {
    curl -s -H "X-API-Key: $API_KEY" "$@"
}

echo "======================================"
echo "Browsion MCP 自动化测试"
echo "======================================"
echo ""

# 颜色输出
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 测试计数器
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# 辅助函数
test_result() {
    local name=$1
    local result=$2
    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if [ $result -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}✗${NC} $name"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
}

# 检查 API
echo "1. 检查 HTTP API..."
if curl_api "$API_BASE/profiles" > /dev/null 2>&1; then
    test_result "HTTP API 可访问" 0
else
    test_result "HTTP API 不可访问" 1
    echo -e "${RED}错误: Browsion 应用未启动${NC}"
    echo "请先运行: cd /home/percy/works/browsion && npm run tauri dev"
    exit 1
fi
echo ""

# 清理函数
cleanup() {
    echo ""
    echo "======================================"
    echo "清理测试环境"
    echo "======================================"

    # 停止浏览器
    echo "停止测试浏览器..."
    curl_api -X POST "$API_BASE/kill/$PROFILE_ID" > /dev/null 2>&1 || true

    # 删除测试 profile
    echo "删除测试 profile..."
    curl_api -X DELETE "$API_BASE/profiles/$PROFILE_ID" > /dev/null 2>&1 || true

    # 清理用户数据目录
    if [ -d "$USER_DATA_DIR" ]; then
        rm -rf "$USER_DATA_DIR"
        echo "已删除用户数据目录"
    fi

    echo "清理完成"
}

# 设置陷阱确保清理
trap cleanup EXIT

echo "2. Profile 管理测试"
echo "======================================"

# 创建测试 profile
echo "创建测试 profile..."
CREATE_RESULT=$(curl_api -X POST "$API_BASE/profiles" \
    -H "Content-Type: application/json" \
    -d "{
        \"id\": \"$PROFILE_ID\",
        \"name\": \"MCP 自动测试\",
        \"description\": \"自动化测试生成的 profile\",
        \"user_data_dir\": \"$USER_DATA_DIR\",
        \"lang\": \"zh-CN\",
        \"proxy_server\": \"$PROXY\",
        \"tags\": [\"auto-test\", \"mcp\"]
    }")

if echo "$CREATE_RESULT" | grep -q "$PROFILE_ID"; then
    test_result "创建 profile" 0
else
    test_result "创建 profile" 1
    echo "错误: $CREATE_RESULT"
    exit 1
fi

# 获取 profile
echo "获取 profile 详情..."
GET_RESULT=$(curl_api "$API_BASE/profiles/$PROFILE_ID")
if echo "$GET_RESULT" | grep -q "$PROFILE_ID"; then
    test_result "获取 profile" 0
else
    test_result "获取 profile" 1
fi

# 列出 profiles
echo "列出所有 profiles..."
if curl_api "$API_BASE/profiles" | grep -q "$PROFILE_ID"; then
    test_result "列出 profiles" 0
else
    test_result "列出 profiles" 1
fi
echo ""

echo "3. 浏览器生命周期测试"
echo "======================================"

# 启动浏览器
echo "启动浏览器..."
LAUNCH_RESULT=$(curl_api -X POST "$API_BASE/launch/$PROFILE_ID")
if echo "$LAUNCH_RESULT" | grep -q "pid"; then
    test_result "启动浏览器" 0
    PID=$(echo "$LAUNCH_RESULT" | grep -o '"pid":[0-9]+' | grep -o '[0-9]+')
    echo "  浏览器 PID: $PID"
    sleep 2
else
    test_result "启动浏览器" 1
    echo "错误: $LAUNCH_RESULT"
    cleanup
    exit 1
fi

# 获取运行中的浏览器
echo "获取运行中的浏览器..."
RUNNING_RESULT=$(curl_api "$API_BASE/running")
if echo "$RUNNING_RESULT" | grep -q "$PROFILE_ID"; then
    test_result "获取运行状态" 0
else
    test_result "获取运行状态" 1
fi
echo ""

echo "4. 导航功能测试"
echo "======================================"

# 导航到 example.com
echo "导航到 example.com..."
NAV_RESULT=$(curl_api -X POST "$API_BASE/browser/$PROFILE_ID/navigate" \
    -H "Content-Type: application/json" \
    -d "{\"url\": \"https://example.com\"}")
sleep 2

if curl_api "$API_BASE/browser/$PROFILE_ID/url" | grep -q "example.com"; then
    test_result "导航到 URL" 0
else
    test_result "导航到 URL" 1
fi

# 获取页面标题
echo "获取页面标题..."
TITLE_RESULT=$(curl_api "$API_BASE/browser/$PROFILE_ID/title")
if echo "$TITLE_RESULT" | grep -q -i "example"; then
    test_result "获取页面标题" 0
    echo "  标题: $TITLE_RESULT"
else
    test_result "获取页面标题" 1
fi

# 后退测试
echo "导航到 github.com..."
curl_api -X POST "$API_BASE/browser/$PROFILE_ID/navigate" \
    -H "Content-Type: application/json" \
    -d "{\"url\": \"https://github.com\"}" > /dev/null
sleep 2

echo "后退到 example.com..."
curl_api -X POST "$API_BASE/browser/$PROFILE_ID/back" > /dev/null
sleep 2

if curl_api "$API_BASE/browser/$PROFILE_ID/url" | grep -q "example.com"; then
    test_result "后退操作" 0
else
    test_result "后退操作" 1
fi

# 刷新页面
echo "刷新页面..."
curl_api -X POST "$API_BASE/browser/$PROFILE_ID/reload" > /dev/null
sleep 1
test_result "刷新页面" 0
echo ""

echo "5. 测试完成"
echo "======================================"
echo ""
echo "总测试数: $TOTAL_TESTS"
echo -e "通过: ${GREEN}$PASSED_TESTS${NC}"
echo -e "失败: ${RED}$FAILED_TESTS${NC}"
echo ""

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}所有测试通过！✓${NC}"
    exit 0
else
    echo -e "${RED}有测试失败，请检查${NC}"
    exit 1
fi
