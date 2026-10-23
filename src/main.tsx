import { render } from '@solidjs/web'
import App from './App'
import ErrorBoundary from './components/ErrorBoundary'
import { installGlobalErrorHandlers } from './bridge'
import './styles.css'

// 先装全局兜底，再渲染：这样连首次挂载抛的错也能落进日志
installGlobalErrorHandlers()

render(
  () => (
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  ),
  document.getElementById('app')!,
)
