import type { Rect } from "./document";
import type { RasterDocument } from "./document";

/**
 * 撤销/重做栈：以"区域快照"为单位——每次提交记录改动区域的
 * before/after 字节，内存占用与改动面积成正比而非整图 × 步数。
 */
export interface RegionEdit {
  rect: Rect;
  before: Uint8ClampedArray;
  after: Uint8ClampedArray;
}

const MAX_HISTORY = 100;
/** 区域快照内存预算（64MB）：超出时丢弃最旧记录。 */
const MAX_HISTORY_BYTES = 64 * 1024 * 1024;

export class RasterHistory {
  private readonly doc: RasterDocument;
  private undoStack: RegionEdit[] = [];
  private redoStack: RegionEdit[] = [];
  private bytes = 0;

  constructor(doc: RasterDocument) {
    this.doc = doc;
  }

  get canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  get canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  push(edit: RegionEdit): void {
    this.undoStack.push(edit);
    this.bytes += edit.before.length + edit.after.length;
    this.redoStack = [];
    while (
      this.undoStack.length > MAX_HISTORY ||
      this.bytes > MAX_HISTORY_BYTES
    ) {
      const dropped = this.undoStack.shift();
      if (!dropped) break;
      this.bytes -= dropped.before.length + dropped.after.length;
    }
  }

  undo(): RegionEdit | null {
    const edit = this.undoStack.pop();
    if (!edit) return null;
    this.doc.restoreRegion(edit.rect, edit.before);
    this.redoStack.push(edit);
    return edit;
  }

  redo(): RegionEdit | null {
    const edit = this.redoStack.pop();
    if (!edit) return null;
    this.doc.restoreRegion(edit.rect, edit.after);
    this.undoStack.push(edit);
    return edit;
  }
}
