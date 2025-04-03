# A Rust Console ASCII Editor Application

北京理工大学第三届十行代码挑战赛参赛作品 (赛道一 复杂版)

## 功能实现

- 完整的文本编辑功能
  - 打开和保存文本文件，支持 UTF-8 编码和大部分 emoji 🤗（不支持阿拉伯文和部分新版 emoji）
  - 随意选择、插入、删除、移动和修改文本
  - 完整的键盘+鼠标支持，操作逻辑与 VSCode 相同
  - 支持剪贴板复制粘贴
  - 实现增量历史记录，支持撤销和重做
  - (TODO) 文本搜索替换
- 基本语法高亮支持
  - 支持多种语言，如 Rust、C++、Python 等
  - 支持数字、字符串、注释、语言关键字的高亮
- 友好的终端用户界面（TUI）
  - 行号和状态栏显示
  - 支持键盘+鼠标操作的模态窗口
    - Confirm: 未保存提示、文件覆盖提示
    - Prompt: 文件名输入框
    - Alert: 错误警告框
    - Button 和 Input: 基本组件
- (TODO) 支持自定义配置文件
  - (TODO) 炫酷的设置菜单
  - (TODO) 自定义颜色主题

## 项目结构

```
arcaea
├── src
│   ├── main.rs       # 程序入口，负责处理命令行参数
│   ├── editor.rs     # 程序主实现，又臭又长
│   ├── error.rs      # 程序错误接口定义，主要用来捂编译器的嘴
│   ├── row.rs        # 一行文本的简单封装，用于计算视觉宽度
│   ├── history.rs    # 编辑器历史记录，支持撤销和重做
│   ├── style.rs      # 颜色主题和样式定义
│   ├── syntax.rs     # 读取语法高亮文件
│   ├── terminal.rs   # 终端渲染封装
│   ├── tui.rs        # TUI 组件库
│   └── lib.rs        # 各种导入导出之类的
├── syntax.d/         # 语法高亮配置文件
├── build.rs          # 构建脚本（自动生成版本号）
├── Cargo.toml        # 项目配置
└── README.md         # 自述文件
```

## 运行环境

- Rust 1.70.0+
- 只要支持 ANSI 转义序列以及 24 位彩色的终端都可以运行
  - Windows 10/11 (Windows Terminal)
  - WSL / Linux (Bash, Zsh, Fish)
  - MacOS (未测试)
- 使用 Nerd Font 字体以获得最佳体验

## 使用说明

```bash
arcaea [filename]     # 打开指定文件
arcaea                # 创建新文件
arcaea -v, --version  # 显示版本信息
arcaea -h, --help     # 显示帮助信息
```

## 快捷键

- 方向键 / `PgDn` / `PgUp` / `Home` / `End`: 光标移动
- `Shift` + 方向键 / 鼠标左键拖动: 选择文本
- `Ctrl` + 左右: 光标移动至单词边界
- 鼠标滚轮 / `Ctrl` + 上下: 光标不动，视图移动
- `Alt` + 鼠标滚轮: 快速移动视图
- `Alt` + 上下: 向上/向下移动选中行
- `Shift` + `Alt` + 上下: 向上/向下复制选中行
- `Ctrl+A`: 全选
- `Ctrl+X`: 剪切选中内容（未选中则剪切光标所在行）
- `Ctrl+C` / 右键(选中): 复制选中内容（未选中则复制光标所在行）
- `Ctrl+V` / 右键(未选中): 粘贴剪贴板内容
- `Ctrl+Z`: 撤销
- `Ctrl+Y`: 重做
- `Ctrl+S`: 保存
- `Shift+F12`: 另存为
- `ESC` / `Ctrl+W`: 退出编辑器
- 鼠标左键拖动行标: 选择整行

## 附注

### AI 生成部分

本项目全程使用了 Github Copilot。

Copilot 主要用于以下用途：

1. 根据 Rust 编译器的各种报错对代码进行修改，绝大部分都是没加 `.clone()` 的各种所有权问题。
2. 代码审计和早期 git 提交（后期的中文 commit message 是自己写的。对于越来越大的项目 Copilot 还是不太能理解我的更改）
3. 单元测试

Copilot 没有用于直接生成代码。（其实试过，但是写的简直一坨，放弃了。按 Esc 的次数简直是 Tab 的三倍）

### 既有项目或学科作业部分

本项目不含任何既有项目或学科作业的代码。

### 开源项目引用

本项目使用了以下开源项目：

1. **[kibi](https://github.com/ilai-deutel/kibi)**

  引用并修改了语法高亮的实现。包含以下部分的代码：

  ```
  syntax.d/**
  syntax.rs
  row.rs @ Row::update_syntax 函数
  ```

2. **[crossterm](https://github.com/crossterm-rs/crossterm)**

  终端操作跨平台支持库

3. **[unicode-segmentation](https://github.com/unicode-rs/unicode-segmentation)**

  Unicode 字素分割支持库

4. **[unicode-width](https://github.com/unicode-rs/unicode-width)**

  Unicode 视觉宽度计算支持库

5. **[terminal-clipboard](https://github.com/Canop/terminal-clipboard)**

  剪贴板操作支持库

### 后记

各种意义上的好难写，好多好多的边界情况要处理，满打满算写了刚好一周QAQ

本来找到一个教程就想照着写的，但是发现它实现的太简陋了，决定不看教程自己重头实现，最后写差不多了发现跟教程简直雷同，连命名都十分甚至九分的相似TwT

`editor.rs` 写的实在是太长了又没法解耦合只能继续往上写史山qwq写到搜索替换之后人都麻了完全不想重构只好继续写下去了qwq

没时间适配 tree-sitter 了只好用简单粗暴的方法搞个语法高亮，每次修改都重新渲染，性能原地爆炸（

## Compilation

### Prerequisites

Make sure you have Rust installed on your machine. You can install Rust using [rustup](https://rustup.rs/).

### Building the Project

To build the project, navigate to the project directory and run:

```bash
cargo build             # For debug build
cargo build --release   # For release build
```

### Running the Editor

To run the text editor, use the following command:

```bash
cargo run               # Open a new file
cargo run -- README.md  # Open this README file
```

## Contributing

Feel free to contribute to this project by submitting issues or pull requests.

## License

This project is licensed under the MIT License.
