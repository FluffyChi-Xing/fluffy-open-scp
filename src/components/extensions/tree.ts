export interface FTreeNode {
  key: string
  label: string
  icon?: string
  disabled?: boolean
  selectable?: boolean
  checkable?: boolean
  children?: FTreeNode[]
}

export function flattenTree(nodes: readonly FTreeNode[]): FTreeNode[] {
  return nodes.flatMap((node) => [node, ...flattenTree(node.children ?? [])])
}

export function descendantKeys(node: FTreeNode): string[] {
  return flattenTree([node]).filter((item) => !item.disabled && item.checkable !== false).map((item) => item.key)
}

export function toggleChecked(nodes: readonly FTreeNode[], checkedKeys: readonly string[], key: string, checked: boolean, strict: boolean) {
  const node = flattenTree(nodes).find((item) => item.key === key)
  if (!node || node.disabled || node.checkable === false) return [...checkedKeys]
  const affected = strict ? [key] : descendantKeys(node)
  const next = new Set(checkedKeys)
  affected.forEach((item) => checked ? next.add(item) : next.delete(item))
  return [...next]
}

export function checkState(node: FTreeNode, checkedKeys: ReadonlySet<string>) {
  const keys = descendantKeys(node)
  const checked = keys.filter((key) => checkedKeys.has(key)).length
  return { checked: checked > 0 && checked === keys.length, indeterminate: checked > 0 && checked < keys.length }
}
