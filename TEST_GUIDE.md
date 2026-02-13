# Browsion 测试指南

## 快速开始

### 1. 编译检查

```bash
cd /home/percy/works/browsion

# 检查 Rust 编译
cd src-tauri && cargo build
cd ..

# 检查 TypeScript
npm run build
```

### 2. 运行开发模式

```bash
# 方式 1: 使用 npm script (推荐)
npm run tauri dev

# 方式 2: 分别启动
# 终端 1:
npm run dev

# 终端 2:
cd src-tauri && cargo run
```

### 3. 首次使用

应用启动后:

1. **检查托盘图标**
   - 在系统托盘查找 Browsion 图标
   - 点击图标应该显示/隐藏主窗口

2. **配置 Chrome 路径**
   - 打开 Settings 标签
   - 设置 Chrome 可执行文件路径
   - Linux 默认: `/usr/bin/google-chrome`
   - 保存设置

3. **添加测试配置**
   - 切换到 Profiles 标签
   - 点击 "Add Profile" 按钮
   - 填写以下信息:
     ```
     Name: Test Profile
     Description: My test browser profile
     User Data Dir: /tmp/browsion_test
     Proxy Server: (留空或填写你的代理)
     Language: en-US
     Timezone: America/Los_Angeles
     Fingerprint: 10000
     ```
   - 点击保存

4. **测试启动**
   - 找到刚创建的配置
   - 点击 "Launch" 按钮
   - 观察:
     - 浏览器应该启动
     - 状态应该变为 "Running" (绿色指示器)
     - 控制台应该显示 PID

5. **测试激活**
   - 最小化浏览器窗口
   - 回到 Browsion
   - 点击 "Activate" 按钮
   - 浏览器窗口应该恢复并置顶

6. **测试关闭**
   - 点击 "Kill" 按钮
   - 浏览器应该关闭
   - 状态变回 "Stopped"

## 测试用例

### 测试用例 1: 配置管理

```bash
# 预期: 配置保存到 ~/.config/browsion/config.toml
cat ~/.config/browsion/config.toml
```

应该看到 TOML 格式的配置文件。

### 测试用例 2: 进程追踪

启动一个配置后:

```bash
# 查看进程是否存在
ps aux | grep chrome | grep browsion_test
```

应该看到 Chrome 进程和对应的参数。

### 测试用例 3: 窗口激活 (Linux)

确保安装了窗口管理工具:

```bash
# 检查 wmctrl
which wmctrl

# 如果没有,安装它
sudo pacman -S wmctrl  # Arch/Manjaro
# 或
sudo apt install wmctrl  # Debian/Ubuntu
```

### 测试用例 4: 多配置同时运行

1. 创建 2-3 个配置 (不同的 user-data-dir)
2. 同时启动它们
3. 验证:
   - 每个都有独立的进程
   - 状态都显示为 "Running"
   - 可以分别激活和关闭

### 测试用例 5: 进程清理

1. 启动一个配置
2. 直接从任务管理器/终端杀死 Chrome 进程
3. 等待 10-15 秒
4. 刷新 Browsion 窗口
5. 状态应该自动更新为 "Stopped"

## 调试技巧

### 查看日志

```bash
# 运行时会在终端看到 tracing 日志
# 如果没有看到,可以设置环境变量
RUST_LOG=browsion=debug npm run tauri dev
```

### 检查配置文件

```bash
# 查看配置
cat ~/.config/browsion/config.toml

# 备份配置
cp ~/.config/browsion/config.toml ~/browsion_config_backup.toml

# 重置配置 (删除后重启应用会创建默认配置)
rm ~/.config/browsion/config.toml
```

### 检查进程

```bash
# 查看所有 Chrome 进程
ps aux | grep chrome

# 查看特定配置的进程
ps aux | grep "user-data-dir=/tmp/browsion_test"
```

### 手动测试启动命令

```bash
# 复制 Browsion 生成的命令,手动运行看是否有错误
/usr/bin/google-chrome \
  --user-data-dir=/tmp/browsion_test \
  --fingerprint=10000 \
  --lang=en-US \
  --timezone=America/Los_Angeles
```

## 常见问题

### Q1: 点击 Launch 没有反应

**检查**:
1. Chrome 路径是否正确
2. 终端是否有错误信息
3. user-data-dir 是否有写入权限

**解决**:
```bash
# 验证 Chrome 路径
which google-chrome

# 创建测试目录
mkdir -p /tmp/browsion_test

# 测试手动启动
/usr/bin/google-chrome --user-data-dir=/tmp/browsion_test
```

### Q2: 窗口激活不工作 (Linux)

**检查**:
```bash
which wmctrl
which xdotool
```

**解决**:
```bash
# 安装窗口管理工具
sudo pacman -S wmctrl xdotool
```

### Q3: 托盘图标不显示

**原因**: 一些桌面环境 (如 GNOME 3.26+) 默认不支持托盘图标

**解决** (GNOME):
```bash
# 安装扩展
gnome-extensions install appindicatorsupport@rgcjonas.gmail.com

# 或使用 TopIcons Plus 扩展
```

### Q4: 配置文件不保存

**检查**:
```bash
# 确保配置目录存在
ls -la ~/.config/browsion/

# 检查权限
ls -l ~/.config/browsion/config.toml
```

## 性能基准

预期性能指标:

- 启动时间: < 3 秒
- 内存占用: < 50MB (无浏览器运行时)
- UI 响应: < 200ms
- 状态刷新: 每 5 秒
- 进程清理: 每 10 秒

## 报告 Bug

如果发现问题,请提供:

1. 操作系统和版本
2. 桌面环境
3. Chrome 版本
4. 错误信息 (终端输出)
5. 复现步骤
6. 配置文件内容 (去除敏感信息)

## 下一步测试

### 跨平台测试

- [ ] 在 Windows 上测试
- [ ] 在 macOS 上测试
- [ ] 在不同 Linux 发行版测试

### 压力测试

- [ ] 创建 10+ 个配置
- [ ] 同时运行 5+ 个浏览器
- [ ] 快速启动/关闭循环
- [ ] 长时间运行 (24 小时+)

### 边界情况

- [ ] 无效的 Chrome 路径
- [ ] 不存在的 user-data-dir
- [ ] 无法访问的代理
- [ ] 重复的配置 ID
- [ ] 空配置文件

---

**Happy Testing!** 🚀
