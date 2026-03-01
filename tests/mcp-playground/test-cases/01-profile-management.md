# Profile 管理测试

测试 Browsion MCP 的 Profile 管理功能。

## 前置条件

- Browsion Tauri 应用正在运行
- Claude Code 已连接到 MCP 服务器
- 已创建并导入 `test-profile.json`

## 测试用例

### 1. 列出所有 Profiles

**在 Claude Code 中执行：**

```
请列出所有可用的浏览器 profiles。
```

**预期结果：**
- 返回包含所有 profiles 的列表
- 每个 profile 显示 id, name, is_running 状态

**验证点：**
- ✅ 列表包含我们创建的 `mcp-test-profile`
- ✅ 显示正确的代理配置信息

---

### 2. 获取单个 Profile 详情

**在 Claude Code 中执行：**

```
请获取 mcp-test-profile 的详细信息。
```

**预期结果：**
- 返回 profile 的完整配置
- 包含代理服务器、时区、标签等信息

**验证点：**
- ✅ proxy_server 显示为 `http://192.168.0.220:8889`
- ✅ 时区为 `Asia/Shanghai`
- ✅ tags 包含 `mcp`, `test`, `claude-code`

---

### 3. 创建新 Profile

**在 Claude Code 中执行：**

```
请创建一个新的测试 profile：
- ID: test-profile-temp
- 名称: 临时测试
- 用户数据目录: /tmp/browsion-temp-profile
- 代理服务器: http://192.168.0.220:8889
- 语言: zh-CN
```

**预期结果：**
- Profile 创建成功
- 返回新创建的 profile 信息

**验证点：**
- ✅ Profile 可以在列表中看到
- ✅ 配置与请求一致

---

### 4. 更新 Profile

**在 Claude Code 中执行：**

```
请更新 mcp-test-profile 的配置：
- 添加标签: "updated"
- 更新描述: "已更新的测试环境"
```

**预期结果：**
- Profile 更新成功
- 返回更新后的信息

**验证点：**
- ✅ tags 包含新增的 "updated"
- ✅ 描述已更新

---

### 5. 删除 Profile

**在 Claude Code 中执行：**

```
请删除之前创建的 test-profile-temp。
```

**预期结果：**
- Profile 删除成功
- 列表中不再显示该 profile

**验证点：**
- ✅ 确认删除成功
- ✅ 列表中不再存在

---

## 测试总结

本场景测试 Profile 管理的 5 个核心功能：
- ✅ list_profiles - 列出所有 profiles
- ✅ get_profile - 获取单个 profile 详情
- ✅ create_profile - 创建新 profile
- ✅ update_profile - 更新 profile 配置
- ✅ delete_profile - 删除 profile

**涉及的 MCP 工具：**
- `list_profiles`
- `get_profile`
- `create_profile`
- `update_profile`
- `delete_profile`
