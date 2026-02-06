//! Settings page rendering - Full page endpoints

use crate::AppState;

pub async fn page_settings(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    let config = &state.config;

    let server_host = &config.server.host;
    let server_port = config.server.port;
    let server_auth = &config.server.auth;

    let data_path = &config.data.path;
    let data_main_file = &config.data.main_file;

    let features_budget = config.features.budget_enable;
    let features_time = config.features.time_extraction;

    let pagination_records = config.pagination.records_per_page;

    let inner_content = format!(
        r#"<div class='mb-6'><h2 class='text-2xl font-bold'>设置</h2></div>
        <div class='bg-white rounded-xl shadow-sm p-6 mb-6'>
            <h3 class='text-lg font-semibold mb-4'>服务器设置</h3>
            <div class='grid grid-cols-2 gap-4 mb-4'>
                <div><p class='text-sm text-gray-500'>主机地址</p><p class='font-medium'>{}</p></div>
                <div><p class='text-sm text-gray-500'>端口</p><p class='font-medium'>{}</p></div>
                <div><p class='text-sm text-gray-500'>认证</p><p class='font-medium'>{}</p></div>
            </div>
        </div>
        <div class='bg-white rounded-xl shadow-sm p-6 mb-6'>
            <h3 class='text-lg font-semibold mb-4'>数据设置</h3>
            <div class='grid grid-cols-2 gap-4 mb-4'>
                <div><p class='text-sm text-gray-500'>数据目录</p><p class='font-medium'>{}</p></div>
                <div><p class='text-sm text-gray-500'>主文件</p><p class='font-medium'>{}</p></div>
            </div>
        </div>
        <div class='bg-white rounded-xl shadow-sm p-6 mb-6'>
            <h3 class='text-lg font-semibold mb-4'>功能开关</h3>
            <div class='grid grid-cols-2 gap-4 mb-4'>
                <div><p class='text-sm text-gray-500'>预算管理</p><p class='font-medium'>{}</p></div>
                <div><p class='text-sm text-gray-500'>时间提取</p><p class='font-medium'>{}</p></div>
            </div>
        </div>
        <div class='bg-white rounded-xl shadow-sm p-6'>
            <h3 class='text-lg font-semibold mb-4'>分页设置</h3>
            <div><p class='text-sm text-gray-500'>每页记录数</p><p class='font-medium'>{}</p></div>
        </div>"#,
        server_host,
        server_port,
        if server_auth.is_some() { "已设置" } else { "未设置" },
        data_path.display(),
        data_main_file,
        if features_budget { "启用" } else { "禁用" },
        if features_time { "启用" } else { "禁用" },
        pagination_records
    );

    axum::response::Html(crate::page_response(&headers, "设置", "/settings", &inner_content))
}
