import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';

/**
 * P0-3 spike（priorities.md / blueprint-panel.md §2.2）：
 * 验证 @vue-flow/core 能否在本仓库的 happy-dom 测试环境下渲染。
 * 结论记录于 docs/design/blueprint-panel.md。
 */

// happy-dom 无 ResizeObserver，vue-flow 的尺寸测量依赖它。
class ResizeObserverStub {
  callback: ResizeObserverCallback;
  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
  }
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
vi.stubGlobal('ResizeObserver', ResizeObserverStub);

import FlowSpike from './FlowSpike.vue';

describe('vue-flow happy-dom spike', () => {
  it('mounts, renders node DOM, and exposes the reactive store', async () => {
    const wrapper = mount(FlowSpike, {
      global: {
        stubs: {
          // vue-flow 内部的 Transition/Teleport 在 happy-dom 下无需真实行为
          Transition: false,
        },
      },
    });
    await nextTick();
    await nextTick();
    // 声明式数据经内部 store 生效
    const nodes = wrapper.findComponent({ name: 'VueFlow' }).props('nodes') as unknown[];
    expect(nodes).toHaveLength(2);
    // 容器挂载成功（happy-dom 下容器尺寸为 0，只有良性警告）
    expect(wrapper.find('[data-testid="flow-spike"]').exists()).toBe(true);
    // 节点真实渲染进 DOM
    const nodeEls = wrapper.findAll('.vue-flow__node');
    expect(nodeEls.length).toBe(2);
    wrapper.unmount();
  });
});
