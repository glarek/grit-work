import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('关于页应展示 Grit Work 仓库与上游 Work Review MIT 归属', async () => {
  const source = await readFile(new URL('./About.svelte', import.meta.url), 'utf8');

  assert.match(source, /https:\/\/github\.com\/glarek\/grit-work/);
  assert.match(source, /https:\/\/github\.com\/wm94i\/Work-Review/);
  assert.match(source, /MIT-licens/);
  assert.match(source, /<h1 class="about-brand-title">GRIT Work<\/h1>/);
  assert.match(source, /ExternalLink/);
  assert.doesNotMatch(source, /from 'lucide-svelte'[\s\S]*Github/);
});

test('关于页应禁用自动更新与发布通道控件，并说明手动更新', async () => {
  const source = await readFile(new URL('./About.svelte', import.meta.url), 'utf8');

  assert.doesNotMatch(source, /runUpdateFlow/);
  assert.doesNotMatch(source, /get_update_settings/);
  assert.doesNotMatch(source, /save_update_settings/);
  assert.doesNotMatch(source, /autoCheckUpdate/);
  assert.doesNotMatch(source, /includePrerelease/);
  assert.doesNotMatch(source, /checkForUpdates/);
  assert.match(source, /automatisk uppdatering är avstängd/);
  assert.match(source, /Uppdatera manuellt/);
});

test('关于页应独立读取版本与平台诊断，失败时不得伪造旧版本号', async () => {
  const source = await readFile(new URL('./About.svelte', import.meta.url), 'utf8');

  assert.match(source, /typeof version === 'string' && version\.trim\(\) \? version : '--'/);
  assert.match(source, /get_runtime_platform/);
  assert.doesNotMatch(source, /appVersion = '1\.0\.0'/);
});

test('关于页应保留数据目录入口且不恢复 Linux 会话状态查询', async () => {
  const source = await readFile(new URL('./About.svelte', import.meta.url), 'utf8');

  assert.match(source, /open_data_dir/);
  assert.doesNotMatch(source, /invoke\('get_linux_session_support'\)/);
  assert.doesNotMatch(source, /about\.linuxSessionTitle/);
  assert.doesNotMatch(source, /activeWindowProvider/);
  assert.doesNotMatch(source, /browserUrlSupportLevel/);
});
