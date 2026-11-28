import { i18n } from "@/locales";
import { mount } from "@vue/test-utils";
import { expect, it } from "vitest";
import Node from "./ElementTreeNode.vue";
it("lets the user collapse an automatically expanded ancestor and reopen it", async()=>{
 const root={i:0,parent:-1,tag:"html"};const child={i:1,parent:0,tag:"div"};
 const w=mount(Node,{global:{plugins:[i18n]},props:{node:root,children:new Map([[0,[child]]]),expanded:new Set([0]),selected:1}});
 expect(w.findAll('.element-row')).toHaveLength(2);
 await w.find('.branch').trigger('click');expect(w.findAll('.element-row')).toHaveLength(1);
 expect(w.find('.element-row').attributes('aria-expanded')).toBe('false');
 await w.find('.branch').trigger('click');expect(w.findAll('.element-row')).toHaveLength(2);
 await w.find('.element-row').trigger('keydown',{key:'ArrowLeft'});expect(w.findAll('.element-row')).toHaveLength(1);
 w.unmount();
});
