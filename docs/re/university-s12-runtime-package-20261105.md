# University s12 发布验收

构建输入保留公园菜单，同时显式继承小学功能模板：

```json
{
  "categoryInstance": "0x07049250",
  "menuParentInstance": "0xF970A822",
  "simulationTemplateInstance": "0x7E4B99E5"
}
```

发布前静态检查：

- `building40`：74 meshes、1 material group、参数列 128；与 s9 材质基线一致，未出现 s10 的多材质绑定回退。
- Asset `40E1C000:22AD7BD5` parent = `40E1C000:7E4B99E5`。
- Unit `40E0C000:22AD7BD5` parent = `40E0C000:7E4B99E5`。
- 车辆资源族 = `0AB4DFE1/E2/E3`，`0x0C12EF60=true`。
- PathEntry `00B1B104:40E0C400:03106C2E` 已随 delta 写入，Asset 的 `0x0CC8FE7A` 完全匹配。
- UnitLocation `0x0E1BAC61/62` 数量相等，共 22 个锚点。
- 构建结果 20 entries，SHA-256：
  `39B7BE30DB424016393602C604B140D28204E6276260A836684E0B40CB5CEAC6`。

s11 已归档到 `D:/ea-games/simcity_offline/test-archive/000_OpenSCP_University_s11.package`，
并从 live `SimCityData` 移除。当前 live 目录只部署：

`D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData/000_OpenSCP_University_s12.package`

## s13 correction
The park menu category is now kept separate from Unit behavior. s13 uses menu category 07049250/F970A822, Unit category 95A9D6FD, and parent 7E4B99E5. s12 was archived before deployment.
