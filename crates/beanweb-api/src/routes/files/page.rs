//! Files page rendering - Full page endpoints

use crate::AppState;
use axum::extract::Path;

pub async fn page_files(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    let ledger = state.ledger.read().await;
    let time_range = ledger.time_context().range.to_string();

    let inner_content = r#"<div class='mb-6 flex items-center justify-between'>
            <div><h2 class='text-2xl font-bold'>文件管理</h2><p class='text-gray-500 mt-1'>数据目录: ./data</p></div>
            <input type='text' id='file-search' placeholder='搜索文件...' hx-get='/api/files' hx-trigger='keyup changed delay:300ms' hx-target='#files-list' name='search' class='px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-transparent w-64'>
        </div>
        <div id='files-list' hx-get='/api/files' hx-trigger='load' class='bg-white rounded-xl shadow-sm overflow-hidden'>
            <p class='text-gray-500 text-center py-12'>加载中...</p>
        </div>"#.to_string();

    axum::response::Html(crate::page_response_with_time(&headers, "文件", "/files", &inner_content, &time_range))
}

pub async fn page_file_edit(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: Path<String>,
) -> axum::response::Html<String> {
    let config = &state.config;
    let file_path = path.0;
    let full_path = config.data.path.join(&file_path);

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => String::from("无法读取文件"),
    };

    let line_count = content.lines().count();
    let char_count = content.len();

    // NOTE: 行号功能已注释（显示不准确）
    // let line_numbers_html: String = (1..=line_count.min(500).max(1))
    //     .map(|i| i.to_string())
    //     .collect::<Vec<_>>()
    //     .join("<br>");

    let inner_content = format!(
        r#"<div class='mb-4 flex items-center justify-between'>
            <div class='flex items-center gap-4'>
                <a href='/files' class='text-gray-500 hover:text-gray-700'>← 返回</a>
                <h2 class='text-xl font-bold'>编辑文件 - {}</h2>
            </div>
            <div class='flex items-center gap-2'>
                <span id='save-status' class='text-sm text-gray-500'></span>
                <button onclick="saveFile(this, '{}')" class='px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700'>保存</button>
            </div>
        </div>
        <div id='save-message'></div>
        <div class='bg-white rounded-xl shadow-sm overflow-hidden'>
            <textarea id='file-content' class='w-full p-4 font-mono text-sm resize-none focus:outline-none' style='min-height: calc(100vh - 300px);'>{}</textarea>
        </div>
        <div class='mt-4 text-sm text-gray-500'>行数: {}  字符数: {}  Ctrl+S 保存</div>
        <script>
            function saveFile(btn, path) {{
                const content = document.getElementById('file-content').value;
                btn.disabled = true;
                btn.textContent = '保存中...';
                fetch('/api/files/' + path, {{
                    method: 'PUT',
                    headers: {{'Content-Type': 'text/plain'}},
                    body: content
                }}).then(r => {{
                    if (!r.ok) throw new Error('Save failed');
                    return r.text();
                }}).then(data => {{
                    document.getElementById('save-message').innerHTML = data;
                    btn.disabled = false;
                    btn.textContent = '保存';
                }}).catch(e => {{
                    document.getElementById('save-message').innerHTML = '<div class="bg-red-50 border border-red-200 rounded-lg p-4"><span class="text-red-600">✗</span><span class="font-medium text-red-800">保存失败</span></div>';
                    btn.disabled = false;
                    btn.textContent = '保存';
                }});
            }}
            document.addEventListener('keydown', e => {{
                if ((e.ctrlKey || e.metaKey) && e.key === 's') {{
                    e.preventDefault();
                    const btn = document.querySelector('button[onclick*="saveFile"]');
                    if (btn) btn.click();
                }}
            }});
        </script>"#,
        file_path,
        urlencoding::encode(&file_path),
        content.replace("<", "&lt;").replace(">", "&gt;"),
        line_count,
        char_count
    );

    axum::response::Html(crate::page_response(&headers, "编辑文件", &format!("/files/{}", file_path), &inner_content))
}
