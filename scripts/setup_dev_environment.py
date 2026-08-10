#!/usr/bin/env python3
"""
开发环境自动设置脚本

用途: 自动化新开发者的环境配置
使用: python scripts/setup_dev_environment.py
"""

import os
import platform
import subprocess
import sys
from pathlib import Path
from typing import Optional, Tuple


class Colors:
    """终端颜色输出"""
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    RED = '\033[91m'
    BLUE = '\033[94m'
    RESET = '\033[0m'
    BOLD = '\033[1m'


def print_step(message: str) -> None:
    """打印步骤信息"""
    print(f"{Colors.BLUE}==>{Colors.RESET} {Colors.BOLD}{message}{Colors.RESET}")


def print_success(message: str) -> None:
    """打印成功信息"""
    print(f"{Colors.GREEN}✓{Colors.RESET} {message}")


def print_warning(message: str) -> None:
    """打印警告信息"""
    print(f"{Colors.YELLOW}⚠{Colors.RESET} {message}")


def print_error(message: str) -> None:
    """打印错误信息"""
    print(f"{Colors.RED}✗{Colors.RESET} {message}")


def run_command(
    command: list[str],
    cwd: Optional[Path] = None,
    check: bool = True
) -> Tuple[bool, str]:
    """运行命令并返回结果"""
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            check=check,
            encoding='utf-8',
            errors='replace'
        )
        return True, result.stdout
    except subprocess.CalledProcessError as e:
        return False, e.stderr
    except Exception as e:
        return False, str(e)


def check_command_exists(command: str) -> bool:
    """检查命令是否存在"""
    success, _ = run_command(
        ["where" if platform.system() == "Windows" else "which", command],
        check=False
    )
    return success


def get_version(command: list[str]) -> Optional[str]:
    """获取工具版本"""
    success, output = run_command(command, check=False)
    if success:
        return output.strip().split('\n')[0]
    return None


def check_prerequisites() -> dict[str, dict]:
    """检查所有前置条件"""
    print_step("检查前置条件...")

    prereqs = {
        'git': {
            'command': 'git',
            'version_cmd': ['git', '--version'],
            'required': 'any',
            'install_hint': 'https://git-scm.com/downloads'
        },
        'rust': {
            'command': 'rustc',
            'version_cmd': ['rustc', '--version'],
            'required': '1.88.0',
            'install_hint': 'https://rustup.rs/'
        },
        'cargo': {
            'command': 'cargo',
            'version_cmd': ['cargo', '--version'],
            'required': 'any',
            'install_hint': 'Installed with Rust'
        },
        'python': {
            'command': 'python',
            'version_cmd': ['python', '--version'],
            'required': '3.12.10',
            'install_hint': 'https://www.python.org/downloads/'
        },
        'node': {
            'command': 'node',
            'version_cmd': ['node', '--version'],
            'required': '22.14.0',
            'install_hint': 'https://nodejs.org/'
        },
    }

    results = {}
    for name, info in prereqs.items():
        exists = check_command_exists(info['command'])
        version = get_version(info['version_cmd']) if exists else None

        results[name] = {
            'installed': exists,
            'version': version,
            'required': info['required'],
            'install_hint': info['install_hint']
        }

        if exists:
            print_success(f"{name}: {version}")
        else:
            print_error(f"{name}: 未安装")
            print(f"  安装: {info['install_hint']}")

    return results


def setup_python_venv(root: Path) -> bool:
    """设置 Python 虚拟环境"""
    print_step("设置 Python 虚拟环境...")

    venv_path = root / ".venv"

    if venv_path.exists():
        print_warning(f"虚拟环境已存在: {venv_path}")
        return True

    # 创建虚拟环境
    print("  创建虚拟环境...")
    success, output = run_command(['python', '-m', 'venv', '.venv'], cwd=root)
    if not success:
        print_error(f"创建虚拟环境失败: {output}")
        return False

    print_success("虚拟环境创建成功")

    # 确定 pip 路径
    if platform.system() == "Windows":
        pip_path = venv_path / "Scripts" / "pip.exe"
        python_path = venv_path / "Scripts" / "python.exe"
    else:
        pip_path = venv_path / "bin" / "pip"
        python_path = venv_path / "bin" / "python"

    # 升级 pip
    print("  升级 pip...")
    success, output = run_command(
        [str(python_path), '-m', 'pip', 'install', '--upgrade', 'pip'],
        cwd=root
    )
    if not success:
        print_warning(f"升级 pip 失败: {output}")

    # 安装 sidecar
    print("  安装 sidecar 依赖...")
    success, output = run_command(
        [str(pip_path), 'install', '-e', 'sidecar'],
        cwd=root
    )
    if not success:
        print_error(f"安装 sidecar 失败: {output}")
        return False

    print_success("Python 依赖安装成功")

    # 验证 campaign 模块
    print("  验证 campaign 模块...")
    success, output = run_command(
        [str(python_path), '-c', 'import campaign; print("OK")'],
        cwd=root
    )
    if not success or "OK" not in output:
        print_error(f"campaign 模块导入失败: {output}")
        return False

    print_success("campaign 模块验证成功")
    return True


def initialize_project(root: Path) -> bool:
    """初始化项目"""
    print_step("初始化项目...")

    # 确定 python 路径
    if platform.system() == "Windows":
        python_path = root / ".venv" / "Scripts" / "python.exe"
    else:
        python_path = root / ".venv" / "bin" / "python"

    # 检查项目状态
    success, output = run_command(
        [str(python_path), 'scripts/init_project.py', '--check'],
        cwd=root,
        check=False
    )

    if success:
        print_success("项目已初始化")
        return True

    # 初始化项目
    print("  运行初始化...")
    success, output = run_command(
        [
            str(python_path),
            'scripts/init_project.py',
            '--name', 'MicrocodeX Short Drama Studio'
        ],
        cwd=root
    )

    if not success:
        print_error(f"项目初始化失败: {output}")
        return False

    print_success("项目初始化成功")
    return True


def build_rust_workspace(root: Path) -> bool:
    """构建 Rust workspace"""
    print_step("构建 Rust workspace...")
    print("  这可能需要几分钟...")

    success, output = run_command(
        ['cargo', 'build', '--workspace'],
        cwd=root
    )

    if not success:
        print_error(f"构建失败: {output}")
        return False

    print_success("Rust workspace 构建成功")
    return True


def run_tests(root: Path) -> dict[str, bool]:
    """运行所有测试"""
    print_step("运行测试套件...")

    results = {}

    # 确定 python 路径
    if platform.system() == "Windows":
        python_path = root / ".venv" / "Scripts" / "python.exe"
    else:
        python_path = root / ".venv" / "bin" / "python"

    # Rust workspace 测试
    print("  运行 Rust workspace 测试...")
    success, output = run_command(
        ['cargo', 'test', '--workspace', '--all-features'],
        cwd=root,
        check=False
    )
    results['rust_workspace'] = success
    if success:
        print_success("Rust workspace 测试通过")
    else:
        print_error("Rust workspace 测试失败")
        print(f"  {output[:200]}...")

    # 桌面端测试
    print("  运行桌面端测试...")
    success, output = run_command(
        ['cargo', 'test', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml'],
        cwd=root,
        check=False
    )
    results['desktop'] = success
    if success:
        print_success("桌面端测试通过")
    else:
        print_error("桌面端测试失败")

    # Python sidecar 测试
    print("  运行 Python sidecar 测试...")
    success, output = run_command(
        [str(python_path), '-m', 'unittest', 'discover', '-s', 'sidecar', '-p', 'test_*.py'],
        cwd=root,
        check=False
    )
    results['sidecar'] = success
    if success:
        print_success("Sidecar 测试通过")
    else:
        print_error("Sidecar 测试失败")

    # Python eval 测试
    print("  运行 Python eval 测试...")
    success, output = run_command(
        [str(python_path), '-m', 'unittest', 'discover', '-s', 'eval/tools', '-p', 'test_*.py'],
        cwd=root,
        check=False
    )
    results['eval'] = success
    if success:
        print_success("Eval 测试通过")
    else:
        print_error("Eval 测试失败")

    return results


def print_summary(
    prereqs: dict,
    venv_ok: bool,
    init_ok: bool,
    build_ok: bool,
    test_results: dict[str, bool]
) -> None:
    """打印设置总结"""
    print("\n" + "="*60)
    print(f"{Colors.BOLD}开发环境设置总结{Colors.RESET}")
    print("="*60)

    # 前置条件
    print(f"\n{Colors.BOLD}前置条件:{Colors.RESET}")
    all_installed = all(p['installed'] for p in prereqs.values())
    if all_installed:
        print_success("所有前置工具已安装")
    else:
        print_error("部分工具缺失，请先安装")

    # 环境设置
    print(f"\n{Colors.BOLD}环境设置:{Colors.RESET}")
    print_success("Python 虚拟环境") if venv_ok else print_error("Python 虚拟环境")
    print_success("项目初始化") if init_ok else print_error("项目初始化")
    print_success("Rust 构建") if build_ok else print_error("Rust 构建")

    # 测试结果
    print(f"\n{Colors.BOLD}测试结果:{Colors.RESET}")
    for name, passed in test_results.items():
        print_success(name) if passed else print_error(name)

    # 总体状态
    print(f"\n{Colors.BOLD}总体状态:{Colors.RESET}")
    all_ok = (
        all_installed and
        venv_ok and
        init_ok and
        build_ok and
        all(test_results.values())
    )

    if all_ok:
        print_success("✨ 开发环境设置完成！可以开始开发了。")
        print("\n下一步:")
        print("  1. 阅读 HANDOFF.md 了解当前项目状态")
        print("  2. 阅读 docs/ROADMAP.md 了解开发路线")
        print("  3. 阅读 AGENTS.md 了解开发规则")
        print("  4. 查询 docs/project-memory/PROJECT_REGISTRY.yaml 了解项目结构")
    else:
        print_error("⚠ 部分步骤失败，请检查上面的错误信息")
        print("\n故障排除:")
        print("  1. 查看 TROUBLESHOOTING.md")
        print("  2. 检查前置工具版本是否匹配")
        print("  3. 确保有网络连接（下载依赖）")
        print("  4. 检查磁盘空间是否充足")


def main() -> int:
    """主函数"""
    print(f"\n{Colors.BOLD}MicrocodeX Short Drama Studio{Colors.RESET}")
    print(f"{Colors.BOLD}开发环境自动设置{Colors.RESET}\n")

    # 确定项目根目录
    root = Path(__file__).parent.parent
    print(f"项目根目录: {root}\n")

    # 检查前置条件
    prereqs = check_prerequisites()

    # 如果前置条件不满足，提前退出
    if not all(p['installed'] for p in prereqs.values()):
        print_error("\n请先安装缺失的工具，然后重新运行此脚本")
        return 1

    # 设置 Python 虚拟环境
    venv_ok = setup_python_venv(root)
    if not venv_ok:
        print_error("\nPython 环境设置失败")
        return 1

    # 初始化项目
    init_ok = initialize_project(root)
    if not init_ok:
        print_error("\n项目初始化失败")
        return 1

    # 构建 Rust workspace
    build_ok = build_rust_workspace(root)
    if not build_ok:
        print_error("\nRust 构建失败")
        return 1

    # 运行测试（可选，失败不退出）
    print("\n是否运行测试套件？这可能需要 10-15 分钟。")
    response = input("运行测试? [Y/n]: ").strip().lower()

    if response in ('', 'y', 'yes'):
        test_results = run_tests(root)
    else:
        print_warning("跳过测试")
        test_results = {}

    # 打印总结
    print_summary(prereqs, venv_ok, init_ok, build_ok, test_results)

    return 0 if all([
        all(p['installed'] for p in prereqs.values()),
        venv_ok,
        init_ok,
        build_ok,
    ]) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print_error("\n\n用户中断")
        sys.exit(130)
    except Exception as e:
        print_error(f"\n\n未预期的错误: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
