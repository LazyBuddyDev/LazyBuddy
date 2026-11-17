// Only focus navigation changes the visual input mode; operating a clicked control does not.
export function focusNavigation(node:HTMLElement){
  const document=node.ownerDocument;
  node.dataset.keyboardNavigation='false';
  const keydown=(event:KeyboardEvent)=>{if(event.key==='Tab')node.dataset.keyboardNavigation='true';};
  const pointerdown=()=>{node.dataset.keyboardNavigation='false';};
  const capture={capture:true};
  document.addEventListener('keydown',keydown,capture);
  document.addEventListener('pointerdown',pointerdown,capture);
  return {destroy(){
    document.removeEventListener('keydown',keydown,capture);
    document.removeEventListener('pointerdown',pointerdown,capture);
    delete node.dataset.keyboardNavigation;
  }};
}
