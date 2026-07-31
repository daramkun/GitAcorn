use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};

pub fn file_icons(worktree_path: &str, paths: &[String]) -> HashMap<String, String> {
    let worktree = Path::new(worktree_path);
    paths
        .iter()
        .filter_map(|path| {
            let absolute_path = repository_path(worktree, path)?;
            platform_file_icon(&absolute_path).map(|icon| (path.clone(), icon))
        })
        .collect()
}

fn repository_path(worktree: &Path, relative_path: &str) -> Option<PathBuf> {
    let relative = Path::new(relative_path);
    if relative.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        return None;
    }
    Some(worktree.join(relative))
}

fn png_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

#[cfg(windows)]
fn platform_file_icon(path: &Path) -> Option<String> {
    use std::{ffi::c_void, mem::size_of, os::windows::ffi::OsStrExt, ptr};
    use windows::{
        Win32::{
            Graphics::Gdi::{
                BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
                DIB_RGB_COLORS, DeleteDC, DeleteObject, HGDIOBJ, SelectObject,
            },
            Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            UI::{
                Controls::{IImageList, ILD_TRANSPARENT},
                HiDpi::{
                    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_UNAWARE,
                    SetThreadDpiAwarenessContext,
                },
                Shell::{
                    SHFILEINFOW, SHGFI_SMALLICON, SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES,
                    SHGetFileInfoW, SHGetImageList, SHIL_SMALL,
                },
                WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx, HICON},
            },
        },
        core::PCWSTR,
    };

    const ICON_SIZE: i32 = 16;
    struct ThreadDpiContext(Option<DPI_AWARENESS_CONTEXT>);

    impl Drop for ThreadDpiContext {
        fn drop(&mut self) {
            if let Some(previous) = self.0 {
                unsafe {
                    SetThreadDpiAwarenessContext(previous);
                }
            }
        }
    }

    fn render_icon(hicon: HICON, background: u8) -> Option<Vec<u8>> {
        let mut pixels = ptr::null_mut::<c_void>();
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ICON_SIZE,
                biHeight: -ICON_SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return None;
        }
        let bitmap = unsafe {
            CreateDIBSection(None, &bitmap_info, DIB_RGB_COLORS, &mut pixels, None, 0).ok()
        };
        let Some(bitmap) = bitmap else {
            unsafe {
                let _ = DeleteDC(dc);
            }
            return None;
        };
        let previous = unsafe { SelectObject(dc, HGDIOBJ(bitmap.0)) };
        let pixel_count = (ICON_SIZE * ICON_SIZE) as usize;
        let byte_count = pixel_count * 4;
        if pixels.is_null() {
            unsafe {
                let _ = SelectObject(dc, previous);
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                let _ = DeleteDC(dc);
            }
            return None;
        }
        let bgra = unsafe { std::slice::from_raw_parts_mut(pixels.cast::<u8>(), byte_count) };
        for pixel in bgra.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[background, background, background, 255]);
        }
        let drawn = unsafe {
            DrawIconEx(dc, 0, 0, hicon, ICON_SIZE, ICON_SIZE, 0, None, DI_NORMAL).is_ok()
        };
        let rendered = drawn.then(|| bgra.to_vec());
        unsafe {
            let _ = SelectObject(dc, previous);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
        }
        rendered
    }

    let previous_dpi_context =
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE) };
    let _dpi_context =
        ThreadDpiContext((!previous_dpi_context.0.is_null()).then_some(previous_dpi_context));
    let shell_path = windows_shell_path(path);
    let wide_path = std::ffi::OsStr::new(&shell_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut file_info = SHFILEINFOW::default();
    let mut flags = SHGFI_SYSICONINDEX | SHGFI_SMALLICON;
    if !path.exists() {
        flags |= SHGFI_USEFILEATTRIBUTES;
    }
    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide_path.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut file_info),
            size_of::<SHFILEINFOW>() as u32,
            flags,
        )
    };
    if result == 0 {
        return None;
    }
    let image_list = unsafe { SHGetImageList::<IImageList>(SHIL_SMALL as i32).ok()? };
    let mut frame_width = 0;
    let mut frame_height = 0;
    let has_frame_size = unsafe {
        image_list
            .GetIconSize(&mut frame_width, &mut frame_height)
            .is_ok()
    };
    if !has_frame_size || frame_width != ICON_SIZE || frame_height != ICON_SIZE {
        return None;
    }
    let icon = unsafe {
        image_list
            .GetIcon(file_info.iIcon, ILD_TRANSPARENT.0)
            .ok()?
    };

    let black = render_icon(icon, 0);
    let white = render_icon(icon, 255);
    unsafe {
        let _ = DestroyIcon(icon);
    }
    let (Some(black), Some(white)) = (black, white) else {
        return None;
    };
    let mut rgba = vec![0_u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    for ((black, white), destination) in black
        .chunks_exact(4)
        .zip(white.chunks_exact(4))
        .zip(rgba.chunks_exact_mut(4))
    {
        let background_contribution = (0..3)
            .map(|channel| white[channel].saturating_sub(black[channel]))
            .max()
            .unwrap_or(255);
        let alpha = 255_u8.saturating_sub(background_contribution);
        if alpha > 0 {
            let restore = |channel: usize| {
                ((u16::from(black[channel]) * 255 + u16::from(alpha) / 2) / u16::from(alpha))
                    .min(255) as u8
            };
            destination.copy_from_slice(&[restore(2), restore(1), restore(0), alpha]);
        }
    }

    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, ICON_SIZE as u32, ICON_SIZE as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(png_data_url(&png))
}

#[cfg(windows)]
fn windows_shell_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    let path = if let Some(network_path) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{network_path}")
    } else if let Some(local_path) = path.strip_prefix(r"\\?\") {
        local_path.to_owned()
    } else {
        path.into_owned()
    };
    path.replace('/', r"\")
}

#[cfg(target_os = "macos")]
fn platform_file_icon(path: &Path) -> Option<String> {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSString};

    let path = NSString::from_str(path.to_string_lossy().as_ref());
    let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
    let tiff = image.TIFFRepresentation()?;
    let representation = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let properties = NSDictionary::new();
    let png = unsafe {
        representation.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
    }?;
    Some(png_data_url(&png.to_vec()))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_file_icon(_path: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_paths_reject_parent_and_absolute_components() {
        let root = Path::new("repository");
        assert_eq!(
            repository_path(root, "src/main.rs"),
            Some(root.join("src/main.rs"))
        );
        assert!(repository_path(root, "../secret.txt").is_none());
        assert!(repository_path(root, "/absolute.txt").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_returns_a_png_icon_for_a_file_type() {
        let icon = platform_file_icon(Path::new("gitacorn-icon-probe.txt"))
            .expect("Windows Shell file icon");
        assert!(icon.starts_with("data:image/png;base64,"));
        let png = STANDARD
            .decode(icon.strip_prefix("data:image/png;base64,").unwrap())
            .expect("base64 PNG");
        let mut reader = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .expect("PNG reader");
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).expect("PNG frame");
        assert_eq!((info.width, info.height), (16, 16));
        let pixels = &pixels[..info.buffer_size()];
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_paths_remove_verbatim_prefixes_and_forward_slashes() {
        assert_eq!(
            windows_shell_path(Path::new(r"\\?\C:\Projects\repo/src/main.rs")),
            r"C:\Projects\repo\src\main.rs"
        );
        assert_eq!(
            windows_shell_path(Path::new(r"\\?\UNC\server\share/repo.txt")),
            r"\\server\share\repo.txt"
        );
    }
}
