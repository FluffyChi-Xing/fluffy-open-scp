# OpenSCP Blender 六槽位资产流水线

这套插件把研究记录中的一次性脚本收敛为三个按钮：

1. **Validate slots / 校验槽位**：检查 `mesh.json`、`params.json`、`slot1..5.png`，并生成 manifest。
2. **Build six-slot asset / 构建六槽位资产**：读取探针网格，创建 `UVBase`、`UVTop` 和 `matIndex`；如果指定研究目录的 `b4_shader.py`，使用精确的 B4 节点组，否则使用预览材质。
3. **Export contract GLB / 导出契约 GLB**：重排为 `TEXCOORD_0..3` 并检查 GLB 属性，失败时停止。

## 安装与运行

将 `tools/blender_asset_pipeline` 压缩为 zip，在 Blender 的 Preferences → Add-ons → Install… 安装，启用后在 3D View → N 面板 → OpenSCP 中设置：

* `Asset directory`：例如 `C:/Users/.../科潘大厦/01_原游戏资产/科潘_探针导出`
* `B4 shader.py`：例如 `C:/Users/.../科潘大厦/04_管线脚本/b4_shader.py`
* 输出 GLB 路径

按钮顺序是 **校验 → 构建 → 导出**。白模可以在 Blender 中手工或由 agent 建模；要让它使用原游戏 UV/材质列，应先通过 `building_material_probe` 导出 `mesh.json`。只有白模而没有原 UV 时，仍可把白模替换进场景，但需要额外做 UV/材质列标定，不能自动恢复原资产细节。

## Agent/CI 校验

无需启动 Blender：

```powershell
python -c "from tools.blender_asset_pipeline.core import validate_asset; import json; print(json.dumps(validate_asset(r'...\\科潘_探针导出'), ensure_ascii=False, indent=2))"
```

该流水线不绕过游戏格式或版权边界；它消费你已经合法导出的贴图和网格。`slot0` 参数表必须保留 float 精度，插件会优先从 `params.json` 生成 EXR。
