import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('应用壳层应使用统一 i18n 资源并支持三种界面语言', async () => {
  const [appSource, i18nSource] = await Promise.all([
    readFile(new URL('./App.svelte', import.meta.url), 'utf8'),
    readFile(new URL('./lib/i18n/index.ts', import.meta.url), 'utf8'),
  ]);

  assert.match(i18nSource, /'zh-CN'/);
  assert.match(i18nSource, /'en'/);
  assert.match(i18nSource, /'zh-TW'/);
  assert.match(i18nSource, /'ar'/);
  assert.match(i18nSource, /export function cycleLocale\(\)/);
  assert.match(appSource, /applyLocaleToDocument/);
  assert.match(appSource, /initializeLocale/);
  assert.match(appSource, /\{#key currentLocale\}/);
  assert.match(appSource, /<Router \{routes\} \/>/);
});

test('侧边栏不提供语言切换，关于入口为 Om', async () => {
  const source = await readFile(new URL('./lib/components/Sidebar.svelte', import.meta.url), 'utf8');

  assert.doesNotMatch(source, /locale-switch/);
  assert.doesNotMatch(source, /toggleLocaleMenu/);
  assert.doesNotMatch(source, /selectLocale\(/);
  assert.doesNotMatch(source, /value: 'zh-CN', label: 'ZH'/);
  assert.doesNotMatch(source, /value: 'en', label: 'EN'/);
  assert.doesNotMatch(source, /value: 'zh-TW', label: 'TW'/);
  assert.doesNotMatch(source, /value: 'ar', label: 'AR'/);
  assert.match(source, /label:\s*'Om'/);
  assert.match(source, /GRIT Work/);
});

test('后端语言切换命令应保持私有以避免 Tauri command 宏重导出冲突', async () => {
  const source = await readFile(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8');

  assert.match(source, /#\[tauri::command\]\s*async fn set_app_locale\b/);
  assert.doesNotMatch(source, /#\[tauri::command\]\s*pub async fn set_app_locale\b/);
});

test('侧边栏不含语言菜单展开层', async () => {
  const source = await readFile(new URL('./lib/components/Sidebar.svelte', import.meta.url), 'utf8');

  assert.doesNotMatch(source, /class="absolute bottom-full start-0 mb-2/);
  assert.doesNotMatch(source, /fullLabelKey: 'sidebar\.localeNames\.zhCN'/);
  assert.doesNotMatch(source, /sidebar\.localeNames/);
  assert.doesNotMatch(source, /cycleLocale/);
});

test('桌宠窗口应初始化 locale 并监听语言切换事件', async () => {
  const source = await readFile(new URL('./routes/avatar/AvatarWindow.svelte', import.meta.url), 'utf8');

  assert.match(source, /initializeLocale\(\)/);
  assert.match(source, /applyLocaleToDocument/);
  assert.match(source, /listen\('locale-changed'/);
  assert.match(
    source,
    /getAvatarStateBubble\(\s*nextState\.mode,\s*currentLocale,\s*nextState\.contextLabel,\s*nextState\.avatarPersona,?\s*\)/s
  );
  assert.match(
    source,
    /nextState\.mode !== state\.mode\s*\|\|\s*nextState\.contextLabel !== state\.contextLabel/
  );
});
