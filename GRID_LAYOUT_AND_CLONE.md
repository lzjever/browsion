# 网格布局和克隆功能

## 新功能概述

### 1. 网格布局（Grid Layout）

**目标：** 让卡片以固定大小排列，一行可以显示多个，大幅提高空间利用率

**实现方式：**
```css
.profile-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
  gap: 0.75rem;
  align-items: start;
}
```

**特点：**
- ✅ 卡片宽度固定为 360px（最小）
- ✅ 自动计算每行可以放多少列
- ✅ 响应式设计 - 窗口变大自动增加列数
- ✅ 垂直对齐优化

**容量计算：**

在 1200px 宽度下：
```
可用宽度 = 1200 - (左右 padding: 1.5rem * 2) ≈ 1152px
每列宽度 = 360px + gap 0.75rem ≈ 372px
列数 = 1152 / 372 ≈ 3 列

在 900px 高度下，每列可显示约 8 个卡片
总容量 = 3 列 × 8 行 = 24 个 profile 可见！
```

**对比：**
- 之前（单列）：~8 个 profile
- 现在（3列）：**~24 个 profile** 🚀

### 2. Clone 功能

**目标：** 快速复制现有配置，只修改名称和必要参数

**用户流程：**
1. 点击任意 profile 的 "Clone" 按钮
2. 自动打开编辑表单，所有字段已填充
3. 名称自动添加 " Copy" 后缀
4. 生成新的唯一 ID
5. 用户可以修改任意字段
6. 保存后创建为新的 profile

**技术实现：**

```typescript
// App.tsx - Clone 逻辑
const handleCloneProfile = (profile: BrowserProfile) => {
  const clonedProfile: BrowserProfile = {
    ...profile,         // 复制所有字段
    id: '',            // 清空 ID（表示这是新 profile）
    name: `${profile.name} Copy`,  // 名称加后缀
  };
  setEditingProfile(clonedProfile);
  setShowProfileForm(true);
};

// ProfileForm.tsx - 检测 clone
useEffect(() => {
  if (profile) {
    // 如果 ID 为空，这是 clone 操作，生成新 ID
    const profileData = profile.id ? profile : { ...profile, id: uuidv4() };
    setFormData(profileData);
    setCustomArgsText(profile.custom_args.join('\n'));
  }
}, [profile]);

// 保存时判断
const isClone = profile && !profile.id;
if (profile && profile.id && !isClone) {
  await tauriApi.updateProfile(profileData);  // 更新
} else {
  await tauriApi.addProfile(profileData);     // 新建
}
```

**复制的内容：**
- ✅ 用户数据目录路径
- ✅ 代理服务器
- ✅ 语言设置
- ✅ 时区
- ✅ Fingerprint
- ✅ 颜色标签
- ✅ 自定义参数

**自动修改的内容：**
- 🆔 ID（生成新的 UUID）
- 📝 名称（添加 " Copy" 后缀）

**使用场景：**

**场景 1：创建相似配置**
```
原配置：
- Name: US East Profile
- Proxy: http://192.168.0.220:8889
- Timezone: America/New_York

Clone → 修改：
- Name: US West Profile Copy → US West Profile
- Timezone: America/Los_Angeles
```

**场景 2：测试不同参数**
```
原配置：
- Name: Production Profile
- Custom Args: (empty)

Clone → 修改：
- Name: Test Profile
- Custom Args: --disable-gpu --no-sandbox
```

**场景 3：批量创建**
```
Clone Profile 1000 多次，分别修改：
- user_data_dir: /path/1001, /path/1002, ...
- fingerprint: 1001, 1002, ...
```

### 3. UI 改进

**按钮布局：**
```
[Launch]  [Edit]  [Clone]  [Delete]
  或
[Activate] [Kill] [Edit] [Clone] [Delete]
```

**Clone 按钮样式：**
- 颜色：浅蓝色 (#3498db)
- 位置：Edit 和 Delete 之间
- 大小：与其他按钮一致

**网格卡片特点：**
- 固定宽度：360px+
- 高度：自适应内容
- 间距：0.75rem
- 对齐：顶部对齐（align-items: start）

### 4. 响应式设计

**不同窗口宽度下的列数：**

| 窗口宽度 | 列数 | 可见 Profile（8 行） |
|---------|------|---------------------|
| 900px   | 2列  | ~16 个              |
| 1200px  | 3列  | ~24 个              |
| 1600px+ | 4列  | ~32 个              |

### 5. 文件修改清单

**前端：**
- ✅ `src/App.tsx` - 添加 handleCloneProfile
- ✅ `src/components/ProfileList.tsx` - 传递 onClone 回调
- ✅ `src/components/ProfileItem.tsx` - 添加 Clone 按钮
- ✅ `src/components/ProfileForm.tsx` - 处理 clone 逻辑
- ✅ `src/styles/index.css` - 网格布局 + Clone 按钮样式

**后端：**
- 无需修改（复用现有 add_profile API）

## 测试清单

### 网格布局测试
- [ ] 窗口 1200px 宽时显示 3 列
- [ ] 窗口缩小时自动调整列数
- [ ] 卡片高度自适应内容
- [ ] 间距均匀一致
- [ ] 滚动条正常工作

### Clone 功能测试
- [ ] 点击 Clone 按钮打开表单
- [ ] 表单所有字段已填充原数据
- [ ] 名称自动添加 " Copy" 后缀
- [ ] 可以修改任意字段
- [ ] 保存后创建新 profile
- [ ] 新 profile 有不同的 ID
- [ ] 原 profile 保持不变

### 边界情况
- [ ] Clone 已经是 Copy 的 profile（名称：XXX Copy Copy）
- [ ] Clone 运行中的 profile
- [ ] Clone 后不修改直接保存
- [ ] Clone 多次同一个 profile

## 使用技巧

### 快速批量创建
1. 创建一个模板 profile，设置好通用参数
2. Clone 该 profile
3. 只修改需要变化的字段（如 user_data_dir、fingerprint）
4. 保存
5. 重复 2-4

### 名称管理建议
- Clone 后立即修改名称（删除 " Copy" 后缀）
- 使用有意义的命名（如 "US-001", "UK-002"）
- 利用颜色标签区分类别

## 性能影响

**内存：**
- 网格布局：无额外开销
- Clone 功能：临时复制一个对象，忽略不计

**渲染：**
- 网格布局使用 CSS Grid，性能优秀
- 大量 profile（100+）时仍然流畅

## 未来优化建议

### 网格布局
- [ ] 添加列数切换按钮（2/3/4 列）
- [ ] 记住用户的列数偏好
- [ ] 拖拽排序功能

### Clone 功能
- [ ] Clone 时提供重命名对话框
- [ ] 支持批量 clone（一次 clone N 个）
- [ ] Clone 模板功能

---

**实现日期**: 2026-02-13
**状态**: ✅ 已完成
**测试**: 待测试
