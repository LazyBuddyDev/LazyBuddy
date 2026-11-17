import {test} from 'node:test';
import assert from 'node:assert/strict';
import {focusNavigation} from '../src/focus-navigation.ts';

test('only Tab navigation changes focus mode; pointer input and cleanup reset it',()=>{
  const document=new EventTarget(),dataset:Record<string,string>={};
  const action=focusNavigation({ownerDocument:document,dataset} as unknown as HTMLElement);
  const key=(key:string)=>document.dispatchEvent(Object.assign(new Event('keydown',{cancelable:true}),{key}));
  assert.equal(dataset.keyboardNavigation,'false');
  for(const value of ['ArrowRight','Enter',' ','a']){assert.equal(key(value),true);assert.equal(dataset.keyboardNavigation,'false');}
  key('Tab');assert.equal(dataset.keyboardNavigation,'true');
  key('ArrowLeft');assert.equal(dataset.keyboardNavigation,'true');
  document.dispatchEvent(new Event('pointerdown'));assert.equal(dataset.keyboardNavigation,'false');
  document.dispatchEvent(Object.assign(new Event('keydown'),{key:'Tab',shiftKey:true}));assert.equal(dataset.keyboardNavigation,'true');
  action.destroy();assert.equal(dataset.keyboardNavigation,undefined);
  key('Tab');document.dispatchEvent(new Event('pointerdown'));assert.equal(dataset.keyboardNavigation,undefined);
});
