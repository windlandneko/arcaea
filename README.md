
# A Rust Console ASCII Editor Application
- 基础文本编辑功能
- 用户界面
  - 打开和保存文本文件
  - 支持关键字高亮
  - (TODO) 模态窗口，支持键盘/鼠标操作
  - 支持数字、字符串、注释的高亮
  - 随意插入、删除和修改文本
北京理工大学第三届十行代码挑战赛参赛作品
  - 支持多种编程语言，如 Rust、C++、Python 等
  - 完整的键盘/鼠标支持（快捷键参考VSCode）

  - (TODO) 文本搜索替换
## Features
- (TODO) 基本语法高亮支持

  - 显示行号和状态栏
- (TODO) 支持自定义配置文件
  - 自定义颜色主题
  - 炫酷的设置菜单


## Project Structure
```
arcaea
├── src
│   ├── main.rs      # Entry point of the application
│   ├── editor.rs    # Core editor implementation
│   ├── error.rs     # Error handling
│   ├── row.rs       # Text row management
│   ├── tui.rs       # Terminal UI components
│   └── lib.rs       # Library interface
├── syntax.d/        # Syntax highlighting definitions
├── build.rs         # Build script for version info
├── Cargo.toml       # Project configuration
└── README.md        # Project documentation
```

## Getting Started

### Prerequisites

Make sure you have Rust installed on your machine. You can install Rust using [rustup](https://rustup.rs/).

### Building the Project

To build the project, navigate to the project directory and run:

```
cargo build
```

### Running the Editor

To run the text editor, use the following command:

```
cargo run
```

### Usage

使用方法:
```
arcaea [filename]          # 打开指定文件
arcaea                     # 创建新文件
arcaea -v, --version      # 显示版本信息
arcaea -h, --help         # 显示帮助信息
```

编辑器快捷键:
- 方向键 / `PgDn` / `PgUp` / `Home` / `End`: 不同类型的移动光标
- `Ctrl` + 左右: 移动光标至单词边界
- 鼠标滚轮 / `Ctrl` + 上下: 光标不动，移动视图
- `Alt` + 鼠标滚轮: 快速移动视图
- `Alt` + 上下: 整行移动
- `Ctrl+S`: 保存文件
- `ESC` / `Ctrl+W`: 退出编辑器
- `Shift` + 方向键 / 鼠标左键拖动: 选择文本
- 鼠标左键拖动最左侧的行数: 选择整行

## Contributing

Feel free to contribute to this project by submitting issues or pull requests.

## License

This project is licensed under the MIT License.
