# 设备模拟测试

测试 Browsion MCP 的设备模拟功能。

## 前置条件

- Browsion Tauri 应用正在运行
- Claude Code 已连接到 MCP 服务器
- test-profile 已启动浏览器

## 测试用例

### 1. 设置移动设备视口

**在 Claude Code 中执行：**

```
请模拟 iPhone 13 Pro 设备：
- 视口大小: 390 x 844
- 设备像素比: 3
- 移动设备模式: 开启
```

**预期结果：**
- 视口尺寸改变
- 移动设备特性启用

**验证点：**
- ✅ window.innerWidth 为 390
- ✅ window.innerHeight 为 844
- ✅ user agent 符合移动设备

---

### 2. 设置地理定位

**在 Claude Code 中执行：**

```
请设置浏览器的地理定位为：
- 纬度: 39.9042 (北京)
- 经度: 116.4074
- 精度: 10 米
```

**预期结果：**
- 地理位置设置成功

**验证点：**
- ✅ navigator.geolocation 返回设置的坐标
- ✅ 网页可以获取到正确的位置

---

### 3. 设置 User Agent

**在 Claude Code 中执行：**

```
请设置 User Agent 为：
"Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.0 Mobile/15E148 Safari/604.1"
```

**预期结果：**
- User Agent 更新

**验证点：**
- ✅ navigator.userAgent 反映新值
- ✅ 网站识别为移动设备

---

## 测试总结

本场景测试设备模拟功能：
- ✅ emulate - 综合设备模拟
- ✅ set_viewport - 设置视口大小
- ✅ set_user_agent - 设置 UA
- ✅ set_geolocation - 设置地理定位

**模拟用途：**
- 移动端网页测试
- 地理位置相关功能测试
- 不同设备和浏览器的兼容性测试
- User Agent 检测绕过

**注意事项：**
- 模拟设置会影响所有后续操作
- 重启浏览器会清除模拟状态
- 某些网站可能会检测模拟环境
