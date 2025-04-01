# A Rust Console ASCII Editor Application

北京理工大学第三届十行代码挑战赛参赛作品 (赛道一 复杂版)

## 功能实现

- 完整的文本编辑功能
  - 打开和保存文本文件
  - 随意选择、插入、删除和修改文本
  - 完整的键盘+鼠标支持，操作逻辑与 VSCode 相同
  - 支持剪贴板复制粘贴
  - 增量历史记录，支持撤销和重做
  - (TODO) 文本搜索替换
- (TODO) 基本语法高亮支持
  - 支持多种编程语言，如 Rust、C++、Python 等
  - 支持数字、字符串、注释、语言关键字的高亮
- 友好的终端用户界面
  - 显示行号和状态栏
  - (TODO) 模态窗口，支持键盘+鼠标操作
- (TODO) 支持自定义配置文件
  - 炫酷的设置菜单
  - 自定义颜色主题

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
│   ├── tui.rs        # Terminal UI 组件库
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

## 使用说明

```bash
arcaea [filename]     # 打开指定文件
arcaea                # 创建新文件
arcaea -v, --version  # 显示版本信息
arcaea -h, --help     # 显示帮助信息
```

## **编辑器快捷键**

- 方向键 / `PgDn` / `PgUp` / `Home` / `End`: 光标移动
- `Ctrl` + 左右: 光标移动至单词边界
- 鼠标滚轮 / `Ctrl` + 上下: 光标不动，视图移动
- `Alt` + 鼠标滚轮: 快速移动视图
- `Alt` + 上下: 向上/向下移动选中行
- `Shift` + `Alt` + 上下: 向上/向下复制选中行
- `Ctrl+A`: 全选
- `Ctrl+X`: 剪切选中内容（未选中则剪切光标所在行）
- `Ctrl+C` / 右键(选中): 复制选中内容（未选中则复制光标所在行）
- `Ctrl+V` / 右键(未选中): 粘贴剪贴板内容
- `Ctrl+Z`: 撤销操作
- `Ctrl+Y`: 重做操作
- `Ctrl+S`: 保存文件
- `ESC` / `Ctrl+W`: 退出编辑器
- `Shift` + 方向键 / 鼠标左键拖动: 选择文本
- 鼠标左键拖动行标: 选择整行

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