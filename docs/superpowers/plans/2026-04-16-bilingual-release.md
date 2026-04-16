# Bilingual UI And Auto Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为桌面应用增加中英文切换，并让 GitHub 在主分支提交后自动生成 tag、构建产物、创建 Release，并把 CHANGELOG 最新版本内容显示到 Release 页面。

**Architecture:** 新增一个轻量 i18n 模块统一管理界面文案，配置文件保存语言选择；发布侧新增可本地测试的版本/更新说明提取脚本，由 GitHub Actions 调用脚本决定是否发版和使用哪段 changelog 作为 Release 正文。

**Tech Stack:** Rust, GPUI, GitHub Actions, Python 3 unittest

---

### Task 1: 文案层与配置持久化

**Files:**
- Create: `src/i18n.rs`
- Modify: `src/config.rs`
- Test: `src/i18n.rs`, `src/config.rs`

- [ ] **Step 1: 写语言枚举和文案表的失败测试**
- [ ] **Step 2: 运行语言相关测试，确认先失败**
- [ ] **Step 3: 用最小实现补齐语言枚举、文案查询和配置字段**
- [ ] **Step 4: 再跑语言相关测试，确认通过**

### Task 2: 主界面接入中英文切换

**Files:**
- Modify: `src/app_state.rs`
- Modify: `src/main.rs`
- Test: `cargo test`, `cargo build`

- [ ] **Step 1: 把主界面硬编码文案替换为 i18n 查询**
- [ ] **Step 2: 在标题栏加入语言切换入口并保存配置**
- [ ] **Step 3: 编译验证界面接线正确**

### Task 3: 自动发版与 changelog 提取

**Files:**
- Create: `.github/scripts/release_meta.py`
- Create: `tests/test_release_meta.py`
- Modify: `.github/workflows/build.yml`
- Modify: `Cargo.toml`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 先写 release metadata 脚本测试**
- [ ] **Step 2: 运行 Python 单测，确认先失败**
- [ ] **Step 3: 用最小实现补齐版本校验与 changelog 提取脚本**
- [ ] **Step 4: 修改工作流，接入自动 tag / Release / body**
- [ ] **Step 5: 更新版本号与 changelog 顶部版本**
- [ ] **Step 6: 再跑 Python 单测，确认通过**

### Task 4: 最终验证

**Files:**
- Modify: `src/app_state.rs`
- Modify: `src/config.rs`
- Modify: `src/i18n.rs`
- Modify: `.github/scripts/release_meta.py`
- Modify: `.github/workflows/build.yml`
- Modify: `Cargo.toml`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 运行 `cargo test`**
- [ ] **Step 2: 运行 `cargo build`**
- [ ] **Step 3: 运行 `cargo build --release`**
- [ ] **Step 4: 运行 `python3 -m unittest tests/test_release_meta.py -v`**
- [ ] **Step 5: 运行 `cargo run` 做启动验证**
