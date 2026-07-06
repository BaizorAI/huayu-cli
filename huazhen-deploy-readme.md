huazhen-deploy.sh usage:

# 完整编译 + 部署 (Windows + Linux)
bash huazhen-deploy.sh

# 跳过某步编译
bash huazhen-deploy.sh --skip-win    # 只重新编译 Linux
bash huazhen-deploy.sh --skip-linux  # 只重新编译 Windows
bash huazhen-deploy.sh --skip-build  # 只打包 + 部署（已有二进制）

流程：
1. cargo build --release — Windows exe
2. package.ps1 -SkipBuild — 打包 Windows zip
3. robocopy 同步源码到 C:\wsl-build\todo-app
4. WSL cargo build --release --target x86_64-unknown-linux-gnu
5. WSL package.sh — 打包 Linux tar.gz，复制到 release/
6. scp 两个包 + huazhen-version.txt 到 baizor:/lucky/NewApi/data/install/

注意： 脚本自动用 cygpath 转换 bash 路径为 Windows 路径供 powershell.exe 使用，并在每次运行时重写 C:\wsl-build\package.sh（保持幂等）。