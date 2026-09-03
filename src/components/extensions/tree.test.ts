import { describe, expect, it } from 'vitest'
import { checkState, descendantKeys, toggleChecked, type FTreeNode } from './tree'

const nodes: FTreeNode[] = [{
  key: 'root',
  label: 'Root',
  children: [
    { key: 'read', label: 'Read' },
    { key: 'write', label: 'Write' },
    { key: 'disabled', label: 'Disabled', disabled: true }
  ]
}]

describe('tree helpers', () => {
  it('excludes disabled nodes from selectable descendants', () => {
    expect(descendantKeys(nodes[0])).toEqual(['root', 'read', 'write'])
  })

  it('cascades checked state and reports partial parents', () => {
    expect(toggleChecked(nodes, [], 'root', true, false)).toEqual(['root', 'read', 'write'])
    expect(checkState(nodes[0], new Set(['read']))).toEqual({ checked: false, indeterminate: true })
    expect(checkState(nodes[0], new Set(['root', 'read', 'write']))).toEqual({ checked: true, indeterminate: false })
  })

  it('only changes the requested node in strict mode', () => {
    expect(toggleChecked(nodes, [], 'root', true, true)).toEqual(['root'])
  })
})
