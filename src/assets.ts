// 这个文件曾经负责把 blob 文件名拼成 asset 协议 URL。
//
// 现在缩略图由后端直接解码 + 缩放 + 内联为 data URL（见 `state.rs` 的
// `thumbnail_data_url`），前端拿到 `imageUrl` 就能直接放进 `<img src>`，
// 不再需要拼路径，也不再依赖 asset 协议的 scope / CSP / capabilities 三者同时正确。
//
// 保留这个空模块是为了不让旧的 import 立刻编译失败。确认没有引用后可以删除。
export {}
