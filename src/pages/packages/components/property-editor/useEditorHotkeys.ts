import { onBeforeUnmount, onMounted, type Ref } from "vue";

/**
 * 编辑器 scoped 热键管理器（PE-重构-3）：仅在 active（如编辑器 sheet
 * 打开且会话就绪）时挂载 window keydown（capture），不污染其它页面。
 * 事件目标为可编辑元素（input/textarea/select/contentEditable）时忽略，
 * Escape 除外（允许随时降级选中/关弹层）。
 */

export interface HotkeyBinding {
  /** 组合键："g"、"ctrl+c"、"ctrl+shift+z"、"escape"、"delete"。 */
  combo: string;
  handler: (event: KeyboardEvent) => void;
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select";
}

/** combo 解析：修饰键集合 + 主键（e.key 小写）。 */
export function matchCombo(combo: string, event: KeyboardEvent): boolean {
  const parts = combo.toLowerCase().split("+").map((part) => part.trim());
  const key = parts[parts.length - 1];
  const wantCtrl = parts.includes("ctrl");
  const wantShift = parts.includes("shift");
  const wantAlt = parts.includes("alt");
  if (event.ctrlKey !== wantCtrl) return false;
  if (event.altKey !== wantAlt) return false;
  // shift 的判定放行任意 shift 状态会误伤字母键（Shift+G 也是 g），
  // 显式要求 shift 时严格比对；未要求时只看主键。
  if (wantShift && !event.shiftKey) return false;
  return event.key.toLowerCase() === key;
}

export function useEditorHotkeys(
  active: Ref<boolean>,
  bindings: () => HotkeyBinding[],
) {
  function onKeyDown(event: KeyboardEvent) {
    if (!active.value) return;
    const editable = isEditableTarget(event.target);
    for (const binding of bindings()) {
      if (!matchCombo(binding.combo, event)) continue;
      if (editable && binding.combo.toLowerCase() !== "escape") return;
      event.preventDefault();
      binding.handler(event);
      return;
    }
  }

  onMounted(() => window.addEventListener("keydown", onKeyDown, true));
  onBeforeUnmount(() => window.removeEventListener("keydown", onKeyDown, true));
}
