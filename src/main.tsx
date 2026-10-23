import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import ErrorBoundary from './components/ErrorBoundary'
import { installGlobalErrorHandlers } from './bridge'
import './styles.css'

// 先装全局兜底，再渲染：这样连首次挂载抛的错也能落进日志
installGlobalErrorHandlers()

ReactDOM.createRoot(document.getElementById('app')!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
)
