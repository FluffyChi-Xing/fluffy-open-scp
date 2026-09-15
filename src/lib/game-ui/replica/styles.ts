/**
 * 复刻站样式装载：把 ui_replica 的 11 个样式表按 index.html 的顺序注入 ShadowRoot。
 *
 * 文件内容逐字复制自 probe_fire/ui_replica/css/，不做任何改写；Shadow DOM 保证
 * 游戏样式（::-webkit-scrollbar、* 通配等全局规则）不泄漏到应用其余部分，
 * 应用样式也不会渗进舞台（继承起点在宿主元素上钉住浏览器默认值）。
 */
const REPLICA_BASE = "/game-ui/replica/";

/** 与 ui_replica/index.html 的 <link> 顺序一致。 */
const STYLE_FILES = [
  "css/game/base.css",
  "css/game/Animate.css",
  "css/game/ButtonStyles.css",
  "css/game/TextStyles.css",
  "css/game/TextInputStyles.css",
  "css/game/WindowStyles.css",
  "css/game/editorBody.css",
  "css/game/resizeBox.css",
  "css/hud.css",
  "css/replica.css",
  "css/tweaks.css",
] as const;

export function injectReplicaStyles(root: ShadowRoot): Promise<void> {
  const loaded = STYLE_FILES.map(
    (file) =>
      new Promise<void>((resolve) => {
        const link = document.createElement("link");
        link.rel = "stylesheet";
        link.href = REPLICA_BASE + file;
        link.addEventListener("load", () => resolve());
        link.addEventListener("error", () => {
          console.warn(`[uiReplica] 样式缺失（不影响其余渲染）：${file}`);
          resolve();
        });
        root.appendChild(link);
      }),
  );
  return Promise.all(loaded).then(() => undefined);
}
