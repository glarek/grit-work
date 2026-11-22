import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

async function readAboutSource() {
  return readFile(new URL('./About.svelte', import.meta.url), 'utf8');
}

test('关于页应保留左对齐标准页头，并将品牌主体居中组织', async () => {
  const source = await readAboutSource();

  assert.match(source, /class="page-header page-axis-operation"/);
  assert.match(source, /class="page-title-group"/);
  assert.match(source, /class="page-title-badge"/);
  assert.match(source, /class="page-title-copy"/);
  assert.match(source, /t\('about\.pageTitle'\)/);
  assert.match(source, /about-editorial-shell/);
  assert.match(source, /about-minimal-shell/);
  assert.match(source, /about-brand-card/);
  assert.match(source, /about-brand-identity/);
  assert.match(source, /about-brand-title-row/);
  assert.match(source, /about-version-badge/);
  assert.match(source, /about-brand-title-row[\s\S]*?v\{appVersion\}[\s\S]*?<\/div>/);
  assert.match(source, /about-action-row/);
});

test('品牌卡底部只展示诊断与手动更新说明，不再渲染自动更新控件', async () => {
  const source = await readAboutSource();
  const brandStart = source.indexOf('class="page-card about-brand-card"');
  const brandEnd = source.indexOf('</section>', brandStart);
  const brandCard = source.slice(brandStart, brandEnd);

  assert.doesNotMatch(brandCard, /about-update-controls/);
  assert.doesNotMatch(brandCard, /type="checkbox"/);
  assert.match(brandCard, /v\{appVersion\}/);
  assert.match(brandCard, /role="status" aria-live="polite"/);
  assert.doesNotMatch(source, /about-update-(?:panel|grid|details|info)|about-channel-|isUpdateDetailsOpen/);
  assert.doesNotMatch(source.slice(brandEnd), /about-update-banner/);
});

test('产品原则应使用统一外层卡片并保持三项居中内容', async () => {
  const source = await readAboutSource();

  assert.match(source, /class="page-card about-principles-card"/);
  assert.match(source, /about-principles-title/);
  assert.match(source, /about-trust-grid/);
  assert.equal(
    (source.match(/class="about-trust-card\b/g) || []).length,
    3,
    '产品原则应保留三项'
  );
  assert.doesNotMatch(source, /class="page-card about-trust-card"/);
});

test('应移除技术栈页脚并保留精简原则卡片', async () => {
  const source = await readAboutSource();

  assert.doesNotMatch(source, /about-tech-stack/);
  assert.doesNotMatch(source, /about-tech-item/);
  assert.doesNotMatch(source, /about-tech-pill/);
  assert.doesNotMatch(source, /about-action-strip/);
  assert.doesNotMatch(source, /about-system-note/);
  assert.doesNotMatch(source, /about-system-grid/);
  assert.doesNotMatch(source, /about-linux-pills/);
});
