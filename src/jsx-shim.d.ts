// React 19 + @types/react 19 不再提供全局 JSX namespace —— 项目里很多组件
// 写了 `: JSX.Element` 返回类型，全部 import React.JSX 太啰嗦。这里把 React.JSX
// 重新挂回 global，保持现有写法可用。新代码可以不写返回类型让 TS 推断。
import type * as React from "react";

declare global {
  namespace JSX {
    type Element = React.JSX.Element;
    type ElementClass = React.JSX.ElementClass;
    type ElementAttributesProperty = React.JSX.ElementAttributesProperty;
    type ElementChildrenAttribute = React.JSX.ElementChildrenAttribute;
    type LibraryManagedAttributes<C, P> = React.JSX.LibraryManagedAttributes<C, P>;
    type IntrinsicAttributes = React.JSX.IntrinsicAttributes;
    type IntrinsicClassAttributes<T> = React.JSX.IntrinsicClassAttributes<T>;
    type IntrinsicElements = React.JSX.IntrinsicElements;
  }
}
