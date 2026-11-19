import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
test('GRIT Work forces Quiet Pro and light appearance even for legacy settings',async()=>{
 const [app,settings,css]=await Promise.all(['./App.svelte','./routes/settings/Settings.svelte','./app.css'].map(p=>readFile(new URL(p,import.meta.url),'utf8')));
 assert.match(app,/function applyTheme[\s\S]*?theme = 'light'/);
 assert.match(app,/function applyUiVisualStyle[\s\S]*?uiVisualStyle = 'a'/);
 assert.doesNotMatch(settings,/import SettingsAppearance|id:\s*'appearance'/);
 assert.match(css,/box-shadow:none !important/);
 assert.match(css,/Figtree Variable/);assert.match(css,/JetBrains Mono Variable/);
});
