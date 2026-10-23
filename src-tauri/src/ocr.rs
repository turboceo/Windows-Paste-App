//! Windows 原生 WinRT 本地免安装 OCR 引擎
//!
//! 利用 Windows 10/11 内置的 `Windows.Media.Ocr.OcrEngine` API，
//! 纯本地离线运行、不需要外部可执行文件、不需要模型文件、无版权分发风险。

use std::path::Path;

#[cfg(windows)]
pub fn recognize_file(image_path: &Path) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Graphics::Imaging::{BitmapDecoder, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::StorageFile;

    let path_str = image_path.to_str()?;
    let path = HSTRING::from(path_str);

    // 1. 打开存储文件与读取流
    let file = StorageFile::GetFileFromPathAsync(&path).ok()?.get().ok()?;
    let stream = file
        .OpenAsync(windows::Storage::FileAccessMode::Read)
        .ok()?
        .get()
        .ok()?;

    // 2. 解码图片
    let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.get().ok()?;
    let raw_bitmap = decoder.GetSoftwareBitmapAsync().ok()?.get().ok()?;

    // 3. 转换为 OcrEngine 要求的 Bgra8 格式
    let bitmap = SoftwareBitmap::Convert(&raw_bitmap, BitmapPixelFormat::Bgra8).ok()?;

    // 4. 初始化 OCR 引擎（优先使用系统当前用户语言偏好，如中文/英文）
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;

    // 5. 执行识别
    let result = engine.RecognizeAsync(&bitmap).ok()?.get().ok()?;

    // 6. 提取文字结果
    let text = result.Text().ok()?.to_string();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(not(windows))]
pub fn recognize_file(_image_path: &Path) -> Option<String> {
    None
}
