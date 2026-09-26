import type * as ThreeNamespace from "three";

/**
 * 游戏 post 管线 hejlToneMap 接入（P3 第一项，2026-09-27）。
 *
 * 公式逐字来自 app 包 shader 源码库（tmp/hlsl/40212002_*.txt，migration.md
 * §49.2）：
 * ```
 * texColor *= sunSky.mSunColor.w;              // 曝光（实例数据，值未知）
 * float3 x = max(float3(0, 0, 0), texColor - 0.004);
 * outColor = (x*(6.2*x+.5))/(x*(6.2*x+1.7)+0.06);
 * outColor = pow(outColor, gamma);             // 用户 gamma（未知，取 1）
 * ```
 * 源码注释明示 **「Results of Hejl tonemapping is in sRGB space」**——引擎
 * hejl 输出已是 sRGB；而 three 在 tone mapping 之后还会做 linear→sRGB 编码，
 * 直接接入会双重编码（整体洗白）。故末尾 pow(2.2) 把结果转回线性，交给
 * three 的 colorspace_fragment 再编码，净效果 ≈ 引擎输出。
 *
 * 安装方式：three 对 CustomToneMapping 会在所有受光材质里生成
 * `toneMapping() { return CustomToneMapping(color); }`，但 pars chunk 不含
 * 该函数定义——这里整体替换 tonemapping_pars_fragment 提供之。Chunk 是
 * three 模块级全局；非 Tauri 预览/MeshPreview 等页用 NoToneMapping，
 * TONE_MAPPING 宏不定义、此 chunk 不会被包含，无副作用。
 */

let installed = false;

export function installHejlToneMapping(THREE: typeof ThreeNamespace): void {
  if (installed) return;
  installed = true;
  THREE.ShaderChunk.tonemapping_pars_fragment = /* glsl */ `
	uniform float toneMappingExposure;
	// 引擎 hejlToneMap 逐字：曝光（sunSky.mSunColor.w 的替身）→ x=max(0,c-0.004)
	// → 有理 filmic 曲线 → sRGB 域输出；pow(2.2) 回线性防 three 二次编码。
	vec3 CustomToneMapping( vec3 color ) {
		vec3 c = color * toneMappingExposure;
		vec3 x = max( vec3( 0.0 ), c - 0.004 );
		vec3 o = ( x * ( 6.2 * x + 0.5 ) ) / ( x * ( 6.2 * x + 1.7 ) + 0.06 );
		return pow( o, vec3( 2.2 ) );
	}
`;
}
