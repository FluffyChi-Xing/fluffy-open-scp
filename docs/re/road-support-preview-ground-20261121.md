# 桥梁预览地面锚点修复（2026-11-21）

截图资源：SimCity_Game.package，00B1B104:09558B7E:DC6C325F。

根因在资源检查器的示例场景，不在道路模型轴向：roadAppearanceScene.ts 使用 z=0 的示例道路，同时给 createNativeRoads 提供恒为0的地面高度。桥梁同时包含道路相对与地面相对组件，因此两套锚点重叠。

源码证据：SC/cGraphicsPathPart/BuildInstancedModels.c（0x7AB190），121–155行对 mRelativeToGround 查询并加上 groundHeight；mStackToGround 在道路高度下向地面堆叠。SC/cMeshExtrusionComponent/Init.c 的240591623属性读取 mXformRelativeToGround。

真实DC6C325F：320C7479 横梁/底座的 relativeToGround=true，原始Z包围盒[-11.93832,6.523052]，表偏移为0；62E0555F桥柱相对道路，stackToGround=true，偏移Z=-23.5；D0D848CB等桥身组件相对道路。地面和道路都为0时，底座顶端自然高出路面6.523052米。

修复：新增 roadPreviewGround.ts，只对存在向地面堆叠桥柱的示例外观计算预览地面。使用旋转/缩放后的原始组件包围盒，将地面相对组件的顶部置于固定桥身结构最低点以下。无堆叠桥柱的普通道路返回0。模型顶点、原始偏移和通用地图道路装配器不改。

这是一种明确标记的检查器示例地形，不声称还原实际城市地形。界面中英文显示示例地面深度。

验证：DC6C325F计算地面-30.11177米，31B2B922为-29.91177米；625C6B38普通道路仍为0。前两种真实桥梁浏览器截图确认支撑不再穿面，普通道路保持原布局。625C6B38仍报告既有5个未解析组件，不扩张本次修复范围。

3项新回归测试覆盖桥身/地面锚点分离、不改资源顶点、普通道路不变、实例使用scale.x和旋转包围盒；连同既有道路测试共9项通过。截图位于tmp/road-review/after-*.png。
