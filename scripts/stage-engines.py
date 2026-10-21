#!/usr/bin/env python3
"""把 macOS 引擎和依赖库收进 src-tauri/resources/engines，供安装包使用。"""

import os
import shutil
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DEST = os.path.join(ROOT, "src-tauri", "resources", "engines")
IMAGEOPTIM = (
    "/Applications/ImageOptim.app/Contents/Frameworks/"
    "ImageOptimGPL.framework/Versions/A/Resources"
)
IMAGEOPTIM_FILES = [
    "oxipng",
    "pngquant",
    "zopflipng",
    "jpegtran",
    "jpegoptim",
    "gifsicle",
    "svgcleaner",
    "libimageoptimjpeg.dylib",
]
HOMEBREW_TOOLS = ["cwebp", "gif2webp", "cjpeg"]


def run(args):
    subprocess.check_call(args)


def otool_deps(path):
    output = subprocess.check_output(["otool", "-L", path], text=True)
    deps = []
    for line in output.splitlines()[1:]:
        dep = line.strip().split(" (", 1)[0]
        if dep:
            deps.append(dep)
    return deps


def rpaths(path):
    output = subprocess.check_output(["otool", "-l", path], text=True)
    found = []
    lines = output.splitlines()
    for index, line in enumerate(lines):
        if "cmd LC_RPATH" not in line:
            continue
        for follow in lines[index : index + 6]:
            if "path " in follow:
                found.append(follow.split("path ", 1)[1].split(" (", 1)[0].strip())
                break
    return found


def is_system(dep):
    return dep.startswith("/usr/lib/") or dep.startswith("/System/")


def resolve(dep, owner):
    if dep.startswith("@rpath/") or dep.startswith("@loader_path/") or dep.startswith("@executable_path/"):
        name = os.path.basename(dep)
        bases = []
        owner_dir = os.path.dirname(owner)
        for raw in rpaths(owner):
            base = (
                raw.replace("@loader_path", owner_dir)
                .replace("@executable_path", owner_dir)
            )
            bases.append(base)
        if dep.startswith("@loader_path/") or dep.startswith("@executable_path/"):
            bases.insert(0, owner_dir)
        for base in bases:
            candidate = os.path.normpath(os.path.join(base, name))
            if os.path.isfile(candidate):
                return os.path.realpath(candidate)
        return None
    if os.path.isfile(dep):
        return os.path.realpath(dep)
    return None


def collect(path, files, edges):
    real = os.path.realpath(path)
    if real in files:
        return
    files[real] = os.path.basename(real)
    edges[real] = []
    for dep in otool_deps(real):
        if is_system(dep):
            continue
        found = resolve(dep, real)
        if not found or found == real:
            continue
        edges[real].append((dep, found))
        collect(found, files, edges)


def rewrite(target, real, files, edges):
    for dep, found in edges[real]:
        if found not in files:
            continue
        linked = f"@loader_path/{files[found]}"
        if dep != linked:
            run(["install_name_tool", "-change", dep, linked, target])
    if target.endswith(".dylib"):
        run(["install_name_tool", "-id", f"@loader_path/{os.path.basename(target)}", target])
    run(["codesign", "-s", "-", "-f", target])


def main():
    if sys.platform != "darwin":
        print("目前只收集 macOS 引擎。", file=sys.stderr)
        return 1
    missing = [name for name in IMAGEOPTIM_FILES if not os.path.isfile(os.path.join(IMAGEOPTIM, name))]
    if missing:
        print("找不到 ImageOptim 里的引擎：" + ", ".join(missing), file=sys.stderr)
        return 1
    brew_bins = []
    for name in HOMEBREW_TOOLS:
        found = shutil.which(name)
        if not found:
            print(f"找不到 {name}。需要本机已能运行它，打包时才会带上。", file=sys.stderr)
            return 1
        brew_bins.append(found)

    if os.path.isdir(DEST):
        shutil.rmtree(DEST)
    os.makedirs(DEST)

    for name in IMAGEOPTIM_FILES:
        shutil.copy2(os.path.join(IMAGEOPTIM, name), os.path.join(DEST, name))
        os.chmod(os.path.join(DEST, name), 0o755)

    files = {}
    edges = {}
    for binary in brew_bins:
        collect(binary, files, edges)
    used = set(os.listdir(DEST))
    for real, name in list(files.items()):
        if name not in used:
            used.add(name)
            continue
        stem, ext = os.path.splitext(name)
        index = 2
        while f"{stem}-{index}{ext}" in used:
            index += 1
        name = f"{stem}-{index}{ext}"
        files[real] = name
        used.add(name)
    for real, name in files.items():
        target = os.path.join(DEST, name)
        shutil.copy2(real, target)
        os.chmod(target, 0o755)
        rewrite(target, real, files, edges)

    notice = os.path.join(DEST, "NOTICE.txt")
    with open(notice, "w", encoding="utf-8") as handle:
        handle.write(
            "这些命令行工具随 Pare 安装包分发，便于没有单独安装压缩工具的电脑使用。\n"
            "oxipng、pngquant、zopflipng、jpegtran、jpegoptim、gifsicle、svgcleaner、libimageoptimjpeg.dylib\n"
            "来自本机 ImageOptim 的 GPL 资源目录。\n"
            "cwebp、gif2webp、cjpeg 及其动态库来自本机已安装的命令。\n"
        )
    print(f"已收集 {len(os.listdir(DEST))} 个文件到 {DEST}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
