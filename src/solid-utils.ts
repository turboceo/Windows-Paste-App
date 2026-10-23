import { createEffect, onCleanup } from 'solid-js'

/**
 * 挂载后跑一次副作用，并支持返回清理函数。
 *
 * Solid 2 没有 1.x 的 `onMount`，等价写法是两段式 `createEffect`：
 * compute 不读任何信号（因此永不重跑），apply 里执行副作用。
 * apply 返回的函数会在 owner 销毁时执行，正好对应 onMount 的清理语义。
 */
export function mount(effect: () => void | (() => void)): void {
  createEffect(
    () => undefined,
    () => effect(),
  )
}

/** 挂到 window 上，组件销毁时自动解绑。 */
export function useWindowEvent<K extends keyof WindowEventMap>(
  type: K,
  handler: (event: WindowEventMap[K]) => void,
  options?: AddEventListenerOptions,
): void {
  window.addEventListener(type, handler as EventListener, options)
  onCleanup(() => window.removeEventListener(type, handler as EventListener, options))
}

/**
 * 一个可重复触发、组件销毁时自动清理的定时器。
 *
 * 用普通局部变量存句柄而不是信号：句柄本身不参与渲染，
 * 只有它触发的业务状态才需要响应式。
 */
export function useTimeout(): {
  set: (fn: () => void, ms: number) => void
  clear: () => void
} {
  let handle: number | null = null
  const clear = () => {
    if (handle !== null) {
      window.clearTimeout(handle)
      handle = null
    }
  }
  onCleanup(clear)
  return {
    set(fn, ms) {
      clear()
      handle = window.setTimeout(() => {
        handle = null
        fn()
      }, ms)
    },
    clear,
  }
}
