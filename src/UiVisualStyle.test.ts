import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
test('Dashboard theme follows the persistent user preference and uses local Urbanist',async()=>{
 const [app,settings,css,appearance,main]=await Promise.all(['./App.svelte','./routes/settings/Settings.svelte','./dashboard.css','./lib/stores/appearance.ts','./main.ts'].map(p=>readFile(new URL(p,import.meta.url),'utf8')));
 assert.match(app,/applyDashboardTheme\(\$dashboardTheme\)/);
 assert.doesNotMatch(app,/applyTheme\(config.theme|theme = 'light'/);
 assert.match(appearance,/localStorage.setItem/);
 assert.match(appearance,/classList.toggle\('dark'/);
 assert.doesNotMatch(settings,/import SettingsAppearance|id:\s*'appearance'/);
 assert.match(css,/font-family: Urbanist/);
 assert.match(css,/urbanist-latin-variable.woff2/);
 assert.match(main,/import '.\/dashboard.css'/);
});
