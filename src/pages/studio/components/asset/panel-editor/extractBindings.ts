import ts from 'typescript';
/** Select declarations from the user's game JS AST; no game implementation is vendored. */
export function extractBindings(source:string):string {
  const ast=ts.createSourceFile('game.js',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS),constants:string[]=[],extra:string[]=[];
  const wanted=new Set(['simcity.cRolloverUI.prototype.UpdateItemPositions','simcity.GenerateDataBindingQueries','simcity.Private_BuildDataBindingResults','simcity.RequestDataBindingData','simcity.GetDLCRolloverData','simcity.RenderFeedbackGetTextToUse','simcity.RenderUnitFeedbackText','simcity.SetPopupItemIcon','simcity.FormatDecimalPlace','simcity.FormatLargeNumber']);
  function visit(n:ts.Node){
    if(ts.isBinaryExpression(n)&&n.operatorToken.kind===ts.SyntaxKind.EqualsToken){
      const lhs=n.left.getText(ast),rhs=n.right.getText(ast);
      if(/^simcity\.[\w.]+$/.test(lhs)&&/^(?:-?\d+(?:\.\d+)?(?:E\d+)?|"(?:[^"\\]|\\.)*"|!0|!1)$/.test(rhs)){
        const parts=lhs.split('.');for(let i=2;i<parts.length;i++){const parent=parts.slice(0,i).join('.');constants.push(`${parent}=${parent}||{};`);}constants.push(`${lhs}=${rhs};`);
      }
      if(wanted.has(lhs)||lhs.startsWith('simcity.FormatLargeNumber')||lhs.startsWith('scrui.cPopupMenu'))extra.push(n.getText(ast)+';'+(lhs==='scrui.cPopupMenu'?'goog.inherits(scrui.cPopupMenu,scrui.cUIHandler);':''));
    }
    ts.forEachChild(n,visit);
  }visit(ast);
  const section=(start:string,end:string)=>{const a=source.indexOf(start),b=source.indexOf(end,a);if(a<0||b<a)throw Error('PANEL_NATIVE_SCRIPT_SHAPE');return source.slice(a,b);};
  return 'var simcity={};\n'+constants.join('\n')+section('simcity.RolloverItems={};','simcity.GenerateRolloverItemDataBindingMap=function')+'\nsimcity.cRolloverUI.prototype={};\n'+extra.join('\n')+'\n'+section('simcity.PopupItems={};','simcity.GeneratePopupItemDataBindingMap=function');
}
