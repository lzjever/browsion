# MCP 测试用例索引

完整的 MCP 工具测试覆盖索引。

## 测试场景清单

| # | 场景 | 文件 | MCP 工具数量 | 状态 |
|---|------|------|--------------|------|
| 1 | Profile 管理 | [01-profile-management.md](./01-profile-management.md) | 5 | ✅ |
| 2 | 浏览器生命周期 | [02-browser-lifecycle.md](./02-browser-lifecycle.md) | 3 | ✅ |
| 3 | 导航功能 | [03-navigation.md](./03-navigation.md) | 7 | ✅ |
| 4 | 鼠标键盘 | [04-mouse-keyboard.md](./04-mouse-keyboard.md) | 8 | ✅ |
| 5 | 表单交互 | [05-forms-interaction.md](./05-forms-interaction.md) | 3 | ✅ |
| 6 | 标签页管理 | [06-tabs-management.md](./06-tabs-management.md) | 5 | ✅ |
| 7 | Cookie 和存储 | [07-cookies-storage.md](./07-cookies-storage.md) | 8 | ✅ |
| 8 | 截图功能 | [08-screenshot.md](./08-screenshot.md) | 2 | ✅ |
| 9 | 框架和对话框 | [09-frames-dialogs.md](./09-frames-dialogs.md) | 4 | ✅ |
| 10 | 网络拦截 | [10-network-mocking.md](./10-network-mocking.md) | 5 | ✅ |
| 11 | 设备模拟 | [11-emulation.md](./11-emulation.md) | 2 | ✅ |
| 12 | 工作流录制 | [12-workflows-recording.md](./12-workflows-recording.md) | 10 | ✅ |

**总计：62 个 MCP 工具**

## MCP 工具完整列表

### Profile 管理 (5 tools)
- list_profiles
- get_profile
- create_profile
- update_profile
- delete_profile

### 浏览器生命周期 (3 tools)
- launch_browser
- kill_browser
- get_running_browsers

### 导航 (7 tools)
- navigate
- get_current_url
- get_page_title
- go_back
- go_forward
- reload
- wait_for_url

### 鼠标操作 (8 tools)
- click
- hover
- double_click
- right_click
- click_at
- drag
- slow_type
- press_key

### 表单交互 (3 tools)
- type_text
- select_option
- upload_file

### 标签页管理 (5 tools)
- list_tabs
- new_tab
- switch_tab
- close_tab
- wait_for_new_tab

### Cookie 和存储 (8 tools)
- set_cookie
- get_cookies
- delete_cookies
- export_cookies
- import_cookies
- set_storage
- get_storage
- clear_storage

### 截图 (2 tools)
- screenshot
- screenshot_element

### 框架和对话框 (4 tools)
- get_frames
- switch_frame
- main_frame
- handle_dialog

### 网络操作 (5 tools)
- get_network_log
- clear_network_log
- block_url
- mock_url
- clear_intercepts

### 设备模拟 (2 tools)
- emulate
- set_geolocation

### 工作流和录制 (10 tools)
- list_workflows
- get_workflow
- create_workflow
- update_workflow
- delete_workflow
- run_workflow
- start_recording
- stop_recording
- get_recording_status
- recording_to_workflow

## 测试执行顺序建议

1. **基础测试** (先执行这些)
   - Profile 管理
   - 浏览器生命周期

2. **核心功能** (然后测试这些)
   - 导航功能
   - 鼠标键盘
   - 表单交互

3. **高级功能** (最后测试这些)
   - Cookie 和存储
   - 标签页管理
   - 截图
   - 网络拦截
   - 设备模拟
   - 框架和对话框
   - 工作流录制

## 快速测试命令

在 Claude Code 中可以使用的快速测试命令：

```
# 快速功能验证
1. 列出 profiles
2. 启动 mcp-test-profile
3. 导航到 https://example.com
4. 获取页面标题
5. 截图
6. 停止浏览器
```

## 测试结果记录

使用以下格式记录测试结果：

```markdown
## 测试执行记录

**日期：** 2026-03-01
**测试者：** [您的名字]
**MCP 版本：** v0.9.4

### 测试结果汇总

| 场景 | 结果 | 备注 |
|------|------|------|
| Profile 管理 | ✅ 通过 | 所有工具正常 |
| 浏览器生命周期 | ✅ 通过 | 启动/停止正常 |
| 导航功能 | ⏸️ 待测试 | - |
| 鼠标键盘 | ⏸️ 待测试 | - |
| ... | ... | ... |

### 问题记录

| 场景 | 问题 | 解决方案 |
|------|------|----------|
| ... | ... | ... |
```
