// Central string table for the lightweight i18n layer (no i18next dependency).
// Keys are flat dot-paths. `{n}` / `{msg}` etc. are interpolated by `t()`.
// `zh` is the default/source locale; every `en` entry mirrors a `zh` entry.

export const STRINGS = {
  zh: {
    // date groups (utils/grouping)
    "group.pinned": "置顶",
    "group.today": "今天",
    "group.yesterday": "昨天",

    // relative time (utils/time)
    "time.just_now": "刚刚",
    "time.sec_ago": "{n}秒前",
    "time.min_ago": "{n}分钟前",
    "time.hour_ago": "{n}小时前",
    "time.day_ago": "{n}天前",
    "time.week_ago": "{n}周前",

    // settings view
    "settings.title": "设置",
    "settings.section.general": "通用",
    "settings.section.appearance": "外观",
    "settings.section.shortcut": "快捷键",
    "settings.section.data": "数据",
    "settings.autostart": "开机自启",
    "settings.autostart_desc": "关闭后需要通过快捷键唤起应用",
    "settings.menubar_icon": "显示菜单栏图标",
    "settings.default_action": "默认双击操作",
    "settings.action_paste": "直接粘贴 (推荐)",
    "settings.action_copy": "仅复制",
    "settings.language": "语言",
    "settings.shortcut_toggle": "唤起窗口",
    "settings.shortcut_toggle_desc": "全局快捷键，在任意应用中唤起 Magpie",
    "settings.export": "导出",
    "settings.exporting": "导出中…",
    "settings.import": "导入",
    "settings.importing": "导入中…",
    "settings.exported": "已导出 {n} 条记录",
    "settings.export_failed": "导出失败: {msg}",
    "settings.imported": "已导入 {n} 条新记录",
    "settings.import_none": "没有新记录需要导入",
    "settings.import_failed": "导入失败: {msg}",
    "settings.shortcut_updated": "快捷键已更新",
    "settings.export_history": "导出历史记录",
    "settings.export_history_desc": "导出所有记录为 JSON 文件",
    "settings.import_history": "导入历史记录",
    "settings.import_history_desc": "从 JSON 文件导入，自动跳过重复",
    "settings.clear_history": "清空剪贴板历史",
    "settings.clear_history_desc": "已置顶的记录不会被删除",
    "settings.clear": "清空",
    "settings.clear_confirm_title": "确认清空",
    "settings.clear_confirm_desc": "将删除所有未置顶的剪贴板记录，此操作无法撤销。",

    // app init
    "init.loading": "初始化中…",
    "init.db_failed_title": "数据库初始化失败",
    "init.db_failed_desc": "请重启 Magpie；若反复出现，请检查磁盘空间或重新安装。",

    // common
    "common.cancel": "取消",
    "common.confirm": "确认",
    "common.save": "保存",
    "common.loading": "加载中…",

    // settings labels (theme / accent pickers)
    "settings.theme_label": "主题",
    "settings.accent_label": "主题色",

    // edit content modal
    "edit.chars_lines": "{chars} 字符 · {lines} 行",
    "edit.modified": "已修改",

    // search filter dropdown
    "filter.all": "全部类型",
    "filter.text": "文本",
    "filter.image": "图片",
    "filter.file": "文件",
    "filter.url": "链接",
    "filter.color": "颜色",
    "filter.code": "代码",
    "filter.email": "邮箱",
    "search.placeholder": "输入以筛选条目…",

    // action panel
    "action.paste_to": "粘贴到 {app}",
    "action.append": "追加到剪贴板",
    "action.no_match": "没有匹配的操作",

    // content type labels (classifier.getTypeLabel)
    "type.text": "文本",
    "type.url": "链接",
    "type.code": "代码片段",
    "type.image": "图片",
    "type.color": "颜色值",
    "type.email": "邮箱地址",
    "type.file": "文件",

    // preview panel
    "about.title": "关于 Magpie",
    "about.tagline": "极简的跨平台剪贴板管理器",
    "preview.select_entry": "选择一个条目查看详情",
    "preview.cannot_preview": "无法预览(文件可能已被移动或删除)",
    "preview.pdf_title": "PDF 预览",
    "preview.load_failed": "无法加载文本预览。",
    "preview.truncated": "…(预览已截断)",
    "preview.badge_email": "邮箱地址",
    "preview.badge_weblink": "网页链接",
    "preview.badge_code": "代码",
    "preview.info": "信息",
    "preview.today_at": "今天 {time}",
    "preview.yesterday_at": "昨天 {time}",
    "info.source": "来源",
    "info.content_type": "类型",
    "info.path": "路径",
    "info.files": "文件数",
    "info.file_size": "文件大小",
    "info.characters": "字符数",
    "info.words": "词数",
    "info.lines": "行数",
    "info.size": "尺寸",
    "info.times_copied": "复制次数",
    "info.last_copied": "最近复制",
    "info.first_copied": "首次复制",
    "info.copied": "复制于",
    "info.status": "状态",
    "info.pinned": "已置顶",

    // theme picker
    "theme.light": "浅色",
    "theme.dark": "深色",
    "theme.system": "自动",

    // accent colors
    "color.blue": "蓝色",
    "color.purple": "紫色",
    "color.pink": "粉色",
    "color.red": "红色",
    "color.orange": "橙色",
    "color.green": "绿色",
    "color.teal": "青色",

    // clipboard history — toasts
    "toast.copied": "已复制到剪贴板",
    "toast.pasted_kept": "已粘贴（窗口保持打开）",
    "toast.appended": "已追加到剪贴板",
    "toast.content_updated": "内容已更新",
    "toast.deleted": "已删除",
    "toast.history_cleared": "历史已清空",
    "toast.pinned": "已置顶",
    "toast.unpinned": "已取消置顶",

    // clipboard history — empty states
    "empty.no_match_title": "没有找到匹配的内容",
    "empty.no_match_desc": "试试其他关键词",
    "empty.empty_title": "剪贴板历史为空",
    "empty.empty_desc": "复制一些内容开始吧",

    // generic ui
    "ui.settings": "设置",
    "ui.clear_all_history": "清空全部历史",
    "ui.search_placeholder": "搜索…",
    "ui.clear": "清除",
    "ui.filter": "筛选",
    "ui.clear_all": "清空全部",
    "ui.clear_confirm_message": "所有剪贴板历史将被永久删除，已置顶的条目会保留。",

    // action panel
    "action.search_placeholder": "搜索操作…",
    "action.paste": "粘贴",
    "action.copy": "复制到剪贴板",
    "action.paste_plain": "以纯文本粘贴",
    "action.paste_keep": "粘贴并保持窗口打开",
    "action.open_browser": "在浏览器中打开",
    "action.edit": "编辑内容",
    "action.pin": "置顶",
    "action.unpin": "取消置顶",
    "action.save_file": "保存为文件…",
    "action.delete": "删除",
    "action.clear_all": "清空全部历史",

    // clipboard item / preview
    "item.empty": "(空)",
    "item.files_summary": "{names} 等 {n} 个文件",
    "preview.no_text": "(无文本内容)",

    // about
    "about.name": "名称",
    "about.version": "版本",
    "about.developer": "开发者",
    "about.repo": "开源仓库",

    // errors (parseAppError fallbacks)
    "error.db_unavailable": "数据库不可用",
  },
  en: {
    "group.pinned": "Pinned",
    "group.today": "Today",
    "group.yesterday": "Yesterday",

    "time.just_now": "just now",
    "time.sec_ago": "{n}s ago",
    "time.min_ago": "{n}m ago",
    "time.hour_ago": "{n}h ago",
    "time.day_ago": "{n}d ago",
    "time.week_ago": "{n}w ago",

    "settings.title": "Settings",
    "settings.section.general": "General",
    "settings.section.appearance": "Appearance",
    "settings.section.shortcut": "Shortcuts",
    "settings.section.data": "Data",
    "settings.autostart": "Launch at login",
    "settings.autostart_desc": "When closed, summon the app with the global shortcut",
    "settings.menubar_icon": "Show menu bar icon",
    "settings.default_action": "Default double-click action",
    "settings.action_paste": "Paste (recommended)",
    "settings.action_copy": "Copy only",
    "settings.language": "Language",
    "settings.shortcut_toggle": "Toggle window",
    "settings.shortcut_toggle_desc": "Global shortcut to summon Magpie from any app",
    "settings.export": "Export",
    "settings.exporting": "Exporting…",
    "settings.import": "Import",
    "settings.importing": "Importing…",
    "settings.exported": "Exported {n} items",
    "settings.export_failed": "Export failed: {msg}",
    "settings.imported": "Imported {n} new items",
    "settings.import_none": "No new items to import",
    "settings.import_failed": "Import failed: {msg}",
    "settings.shortcut_updated": "Shortcut updated",
    "settings.export_history": "Export history",
    "settings.export_history_desc": "Export all records as a JSON file",
    "settings.import_history": "Import history",
    "settings.import_history_desc": "Import from a JSON file, skipping duplicates",
    "settings.clear_history": "Clear clipboard history",
    "settings.clear_history_desc": "Pinned records won't be deleted",
    "settings.clear": "Clear",
    "settings.clear_confirm_title": "Confirm clear",
    "settings.clear_confirm_desc": "This deletes all unpinned clipboard records and cannot be undone.",

    // app init
    "init.loading": "Initializing…",
    "init.db_failed_title": "Database initialization failed",
    "init.db_failed_desc": "Please restart Magpie; if it persists, check disk space or reinstall.",

    // common
    "common.cancel": "Cancel",
    "common.confirm": "Confirm",
    "common.save": "Save",
    "common.loading": "Loading…",

    // settings labels (theme / accent pickers)
    "settings.theme_label": "Theme",
    "settings.accent_label": "Accent color",

    // edit content modal
    "edit.chars_lines": "{chars} characters · {lines} lines",
    "edit.modified": "Modified",

    // search filter dropdown
    "filter.all": "All Types",
    "filter.text": "Text",
    "filter.image": "Images",
    "filter.file": "Files",
    "filter.url": "Links",
    "filter.color": "Colors",
    "filter.code": "Code",
    "filter.email": "Emails",
    "search.placeholder": "Type to filter entries…",

    // action panel
    "action.paste_to": "Paste to {app}",
    "action.append": "Append to Clipboard",
    "action.no_match": "No matching actions",

    // content type labels (classifier.getTypeLabel)
    "type.text": "Text",
    "type.url": "Link",
    "type.code": "Code Snippet",
    "type.image": "Image",
    "type.color": "Color Code",
    "type.email": "Email Address",
    "type.file": "File",

    // preview panel
    "about.title": "About Magpie",
    "about.tagline": "A minimal cross-platform clipboard manager",
    "preview.select_entry": "Select an entry to view details",
    "preview.cannot_preview": "Can't preview (file may have been moved or deleted)",
    "preview.pdf_title": "PDF Preview",
    "preview.load_failed": "Failed to load text preview.",
    "preview.truncated": "… (preview truncated)",
    "preview.badge_email": "Email Address",
    "preview.badge_weblink": "Web Link",
    "preview.badge_code": "Code",
    "preview.info": "Information",
    "preview.today_at": "Today at {time}",
    "preview.yesterday_at": "Yesterday at {time}",
    "info.source": "Source",
    "info.content_type": "Content type",
    "info.path": "Path",
    "info.files": "Files",
    "info.file_size": "File size",
    "info.characters": "Characters",
    "info.words": "Words",
    "info.lines": "Lines",
    "info.size": "Size",
    "info.times_copied": "Times copied",
    "info.last_copied": "Last copied",
    "info.first_copied": "First copied",
    "info.copied": "Copied",
    "info.status": "Status",
    "info.pinned": "Pinned",

    "theme.light": "Light",
    "theme.dark": "Dark",
    "theme.system": "System",

    "color.blue": "Blue",
    "color.purple": "Purple",
    "color.pink": "Pink",
    "color.red": "Red",
    "color.orange": "Orange",
    "color.green": "Green",
    "color.teal": "Teal",

    "toast.copied": "Copied to clipboard",
    "toast.pasted_kept": "Pasted (window kept open)",
    "toast.appended": "Appended to clipboard",
    "toast.content_updated": "Content updated",
    "toast.deleted": "Deleted",
    "toast.history_cleared": "History cleared",
    "toast.pinned": "Pinned to top",
    "toast.unpinned": "Unpinned",

    "empty.no_match_title": "No matching items",
    "empty.no_match_desc": "Try a different keyword",
    "empty.empty_title": "Clipboard history is empty",
    "empty.empty_desc": "Copy something to get started",

    "ui.settings": "Settings",
    "ui.clear_all_history": "Clear all history",
    "ui.search_placeholder": "Search…",
    "ui.clear": "Clear",
    "ui.filter": "Filter",
    "ui.clear_all": "Clear All",
    "ui.clear_confirm_message": "All clipboard history will be permanently deleted. Pinned items will be kept.",

    "action.search_placeholder": "Search actions…",
    "action.paste": "Paste",
    "action.copy": "Copy to Clipboard",
    "action.paste_plain": "Paste as Plain Text",
    "action.paste_keep": "Paste and Keep Window Open",
    "action.open_browser": "Open in Browser",
    "action.edit": "Edit Content",
    "action.pin": "Pin to Top",
    "action.unpin": "Unpin",
    "action.save_file": "Save as File…",
    "action.delete": "Delete",
    "action.clear_all": "Clear All History",

    "item.empty": "(empty)",
    "item.files_summary": "{names} and {n} files",
    "preview.no_text": "(no text content)",

    "about.name": "Name",
    "about.version": "Version",
    "about.developer": "Developer",
    "about.repo": "Repository",

    "error.db_unavailable": "Database unavailable",
  },
} as const;

/** A translation key (keys are identical across locales). */
export type StringKey = keyof typeof STRINGS["zh"];
