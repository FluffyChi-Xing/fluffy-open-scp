"""Build a clean end-user ZIP from a self-contained release EXE."""
from pathlib import Path
import argparse,zipfile,hashlib,re

def build(version):
    if not re.fullmatch(r'\d+\.\d+\.\d+',version):raise ValueError('版本号须为 x.y.z')
    root=Path(__file__).resolve().parent;release=root/'release'/version;exe=release/'SC5 Asset Packager.exe'
    if not exe.is_file():raise ValueError('请先构建对应版本 EXE：'+str(exe))
    if exe.read_bytes()[:2]!=b'MZ':raise ValueError('发布文件不是有效 Windows EXE')
    folder='SC5 Asset Packager '+version
    readme=f'''SC5 Asset Packager {version} — Windows 便携版

使用方法
1. 将整个压缩包解压到普通文件夹，再双击 SC5 Asset Packager.exe。
2. 无需安装 Python、Tk，也无需复制开发目录中的 engine、vendor_tk、prop_assets 等文件夹；这些运行内容已包含在 EXE 中。
3. 首次使用请在软件里选择自己的 SimCity 游戏根目录。发行包不包含游戏文件，打包时需要读取本机游戏资源。
4. 打开 Blender 插件导出的资产文件夹，完整保留同名 GLB、LOTM、lot.package、props.json、lights.json 等导出文件。
5. 检查预览、菜单位置、造价和维护费，再点击导出 Package。成品位于项目的 output 子目录，完成提示会显示完整路径。

可发给别人使用的文件
直接分享本 ZIP 即可。压缩包内只有 EXE 和本说明，不需要构建日志、测试报告或开发源码。请勿在压缩软件内直接运行 EXE。

说明
支持 Windows 64 位。使用者须有可读取的游戏资源目录；程序不会自动安装 Mod 或覆盖游戏文件。
灯光日夜切换已有用户游戏测试通过；当前版本尚未接入手动停用/供电状态控制，停用后装饰灯可能继续亮。后续将单独标定。
'''
    dest=root/'release'/f'SC5 Asset Packager {version} Windows 便携版.zip'
    with zipfile.ZipFile(dest,'w',zipfile.ZIP_DEFLATED,compresslevel=9) as z:
        z.write(exe,folder+'/SC5 Asset Packager.exe');z.writestr(folder+'/使用说明.txt',readme.encode('utf-8-sig'))
    with zipfile.ZipFile(dest) as z:
        if z.testzip() is not None:raise ValueError('便携压缩包校验失败')
        assert len(z.namelist())==2
    return {'zip':str(dest),'bytes':dest.stat().st_size,'sha256':hashlib.sha256(dest.read_bytes()).hexdigest(),'exeSha256':hashlib.sha256(exe.read_bytes()).hexdigest()}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('version');args=p.parse_args();print(build(args.version))
