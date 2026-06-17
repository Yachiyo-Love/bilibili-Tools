# b 站视频解析下载

一个用 Rust 编写的 B 站视频解析与下载命令行工具，支持下载为 `MP4` 或 `MP3`。

适合这些场景：

- 下载普通视频
- 下载多 P 视频
- 自动识别合集并下载全部内容
- 提取音频并导出为 `MP3`
- 指定清晰度、输出目录、Cookie、`ffmpeg` 路径

## Features

- 支持输入 B 站链接、`BV` 号、`av` 号、纯数字 `aid`
- 支持输出 `mp4` / `mp3`
- 支持清晰度选择：`4k` / `1080p` / `720p` / `360p`
- 多 P 视频默认全部下载
- 合集视频优先检测，识别后自动全量下载
- 支持 Cookie 字符串或 `cookies.txt`
- 自动调用 `ffmpeg` 合并音视频或转码音频
- Windows 下如果本地没有 `ffmpeg`，会自动下载可用版本

## Requirements

- Rust
- Windows PowerShell
- `ffmpeg`

如果本机没有 `ffmpeg`，程序会尝试自动下载。

也可以手动安装：

```powershell
winget install Gyan.FFmpeg
```

如果安装后 `ffmpeg -version` 仍然不可用，重开一个终端再试。

## Quick Start

运行源码：

```powershell
cargo run -- "https://www.bilibili.com/video/BVxxxxxxxxxx"
```

编译发布版：

```powershell
cargo build --release
```

可执行文件位置：

```text
target\release\b站视频解析下载.exe
```

## Usage

下载 MP4：

```powershell
cargo run -- "BVxxxxxxxxxx" --format mp4 --quality 1080p
```

下载 MP3：

```powershell
cargo run -- "BVxxxxxxxxxx" --format mp3
```

下载 4K：

```powershell
cargo run -- "BVxxxxxxxxxx" --format mp4 --quality 4k
```

指定输出目录：

```powershell
cargo run -- "BVxxxxxxxxxx" --output-dir D:\Videos
```

使用 Cookie：

```powershell
$env:BILI_COOKIE = "SESSDATA=...; bili_jct=..."
cargo run -- "BVxxxxxxxxxx" --format mp4 --quality 4k
```

使用 Cookie 文件：

```powershell
cargo run -- "BVxxxxxxxxxx" --cookie-file cookies.txt
```

指定某个分 P：

```powershell
cargo run -- "BVxxxxxxxxxx?p=2" --format mp4
```

或：

```powershell
cargo run -- "BVxxxxxxxxxx" --page 2 --format mp4
```

保留临时文件：

```powershell
cargo run -- "BVxxxxxxxxxx" --keep-temp
```

指定 `ffmpeg` 路径：

```powershell
cargo run -- "BVxxxxxxxxxx" --ffmpeg "D:\Tools\ffmpeg\bin\ffmpeg.exe"
```

## Behavior

### 普通视频

- 单 P：下载当前视频
- 多 P：默认下载全部分 P

### 合集视频

程序会优先检查当前视频是否属于可下载合集。

如果识别为合集，会下载合集中的全部视频；如果不是合集，则按普通视频逻辑处理。

## Cookie 说明

以下场景通常需要登录 Cookie：

- 4K 或更高码率
- 会员/高画质资源
- 年龄限制内容
- 地区限制内容
- 某些接口返回受限的音视频流

支持两种方式：

- `--cookie "SESSDATA=...; bili_jct=..."`
- `--cookie-file cookies.txt`

`cookies.txt` 支持 Netscape 格式，也支持直接放一整段 Cookie 头内容。

## Notes

- MP4 下载通常使用 DASH，视频流和音频流会分别下载后再合并
- MP3 下载会先处理音频流，再通过 `ffmpeg` 转码
- 当请求的清晰度不可用时，会自动退到不高于目标清晰度的最佳可用流
- 文件名会自动清洗非法字符
- 如果目标文件已存在，会自动生成不冲突的新文件名

## Disclaimer

请仅下载你有权访问、保存或备份的内容。

本项目仅供学习 Rust、HTTP 请求、媒体处理和命令行工具开发使用。使用者需自行遵守当地法律法规、平台条款以及版权要求。

## License

License: `MPL-2.0`
