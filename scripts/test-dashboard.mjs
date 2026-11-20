import assert from 'node:assert/strict';
import {mkdir} from 'node:fs/promises';
import {chromium} from 'playwright';
import {installReviewFixture} from './grit-review-fixture.mjs';
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH||'/usr/bin/chromium',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1440,height:1040},reducedMotion:'reduce'});
page.setDefaultTimeout(15000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(installReviewFixture());
const base=process.env.GRIT_TEST_URL||'http://127.0.0.1:5183/';
const out='all-artifacts/dashboard-implemented';await mkdir(out,{recursive:true});
try {
 await page.goto(base+'?review');
 await page.getByText('Från aktivitet till tidrapport').waitFor();
 await page.waitForFunction(()=>document.fonts.check('16px Urbanist'));
 assert.match(await page.locator('.dashboard-duration').innerText(),/1 tim/);
 assert.match(await page.locator('.project-legend').innerText(),/ORBIT/);
 assert.equal(await page.locator('html').evaluate(e=>e.classList.contains('dark')),true);
 assert.equal(await page.getByRole('link',{name:'Översikt',exact:true}).getAttribute('aria-current'),'page');
 const side=await page.locator('.app-shell-sidebar-frame').boundingBox();assert.equal(side.x,0);assert.equal(side.y,0);assert.equal(side.height,1040);
 const capture=await page.locator('.capture-state').boundingBox();assert.ok(capture.y>850,'Capture belongs at sidebar bottom');
 await page.screenshot({path:out+'/01-overview-dark.png'});
 await page.getByRole('button',{name:'Välj Spiris-användare',exact:true}).click();
 const dialog=page.getByRole('dialog');await dialog.getByRole('button',{name:/Fortsätt som Testanvändare/}).waitFor();
 await page.screenshot({path:out+'/02-user-picker-dark.png'});
 await dialog.getByRole('searchbox').fill('Emma');assert.equal(await dialog.locator('.user-choice').count(),1);
 await dialog.locator('.user-choice').click();
 await page.evaluate(()=>window.testSelectUserFailure=true);
 await dialog.getByRole('button',{name:'Fortsätt som Emma Lind'}).click();
 await dialog.getByRole('alert').getByText('Användarbytet misslyckades.').waitFor();
 assert.match(await page.locator('.workspace-profile').innerText(),/Testanvändare/);
 await page.evaluate(()=>window.testSelectUserFailure=false);
 await dialog.getByRole('button',{name:'Fortsätt som Emma Lind'}).click();await dialog.waitFor({state:'hidden'});
 await page.getByRole('heading',{name:'Hej, Emma!',exact:true}).waitFor();
 assert.equal(await page.evaluate(()=>window.testCalls.includes('grit_save_settings')),false,'Profile selection does not overwrite settings');
 assert.equal(await page.evaluate(()=>window.testCalls.includes('grit_submit')),false);
 await page.getByRole('button',{name:'Byt till ljust tema'}).click();
 await page.waitForTimeout(3200);
 await page.screenshot({path:out+'/03-overview-light.png',animations:'disabled'});
 await page.reload();await page.getByText('Från aktivitet till tidrapport').waitFor();
 assert.equal(await page.locator('html').evaluate(e=>e.classList.contains('dark')),false,'Theme persists across reload');
 await page.getByRole('button',{name:'Välj Spiris-användare',exact:true}).click();
 await dialog.locator('.user-choice').first().waitFor();
 await page.screenshot({path:out+'/04-user-picker-light.png'});
 await page.keyboard.press('Escape');
 await page.getByRole('button',{name:'Byt till mörkt tema'}).click();
 // Providers are taken from saved settings, not a fixed generic AI mark.
 for(const [provider,name] of [['grok','Grok'],['agy','Antigravity'],['codex','Codex']]){
  await page.evaluate(async provider=>{const api=window.__TAURI_INTERNALS__;const status=await api.invoke('grit_status');await api.invoke('grit_save_settings',{settings:{...status.settings,provider}});window.dispatchEvent(new Event('grit-changed'));},provider);
  await page.locator('[data-connection-badges]').getByRole('img',{name,exact:true}).waitFor();
 }
 // Exercise the existing hourly chart and date controls in the new grid.
 const bars=page.locator('.activity-hourly-bar-button');
 assert.equal(await bars.count(),24);
 await bars.nth(9).click();assert.equal(await bars.nth(9).getAttribute('aria-pressed'),'true');
 assert.ok(await bars.nth(9).evaluate(e=>getComputedStyle(e).backgroundColor!=='rgba(0, 0, 0, 0)'), 'Hours stay visible when category data is unavailable');
 await page.getByRole('button',{name:'Den här veckan',exact:true}).click();
 await page.getByText('Daglig insats',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Idag',exact:true}).click();
 for(const theme of ['dark','light']){
  if(theme==='light')await page.getByRole('button',{name:'Byt till ljust tema'}).click();
  for(const [path,selector,label] of [['/report','.grit-row','report'],['/timeline','.timeline-page','timeline'],['/settings','.settings-stage-layout','settings']]){
   await page.goto(base+'?review#'+path);
   if(path==='/timeline')await page.locator('.app-shell-main-scroll').getByRole('heading',{name:'Tidslinje',exact:true}).waitFor();
   else await page.locator(selector).first().waitFor();
   await page.screenshot({path:`${out}/05-${label}-${theme}.png`});
  }
 }
 // Profile selection must preserve an unsaved AI model and save with the new user.
 await page.goto(base+'#/settings');
 await page.locator('.settings-tab-rail').getByRole('button',{name:/^AI & rapportering/}).click();
 await page.getByRole('button',{name:'Avancerade AI-inställningar',exact:false}).click();
 await page.getByRole('textbox',{name:'Modell',exact:true}).fill('unsaved-custom-model');
 await page.getByRole('button',{name:'Välj Spiris-användare',exact:true}).click();
 await dialog.getByRole('button',{name:/Emma Lind.*Spiris-användare/}).click();
 await dialog.getByRole('button',{name:'Fortsätt som Emma Lind'}).click();await dialog.waitFor({state:'hidden'});
 assert.equal(await page.getByRole('textbox',{name:'Modell',exact:true}).inputValue(),'unsaved-custom-model');
 await page.getByRole('button',{name:'Spara AI-inställningar',exact:true}).click();
 await page.waitForFunction(()=>window.testCallArgs.some(call=>call.cmd==='grit_save_settings'&&call.args.settings.user_id===2&&call.args.settings.model==='unsaved-custom-model'));
 // Switching identity is blocked while a report has unsaved edits.
 await page.goto(base+'?dirty#/report');await page.locator('.grit-row').first().click();
 await page.getByRole('textbox',{name:'Dagbok',exact:true}).fill('Osparad ändring');
 await page.getByRole('button',{name:'Spara rad',exact:true}).click();
 await page.getByRole('button',{name:'Välj Spiris-användare',exact:true}).click();
 await dialog.getByRole('button',{name:/Emma Lind.*Spiris-användare/}).click();
 await dialog.getByRole('button',{name:'Fortsätt som Emma Lind'}).click();
 await dialog.getByRole('alert').getByText(/Spara dina ändringar/).waitFor();
 assert.match(await page.locator('.workspace-profile').innerText(),/Testanvändare/);
 await page.keyboard.press('Escape');
 // Re-selecting the current profile is a no-op and must not reload an unsaved report.
 await page.getByRole('button',{name:'Välj Spiris-användare',exact:true}).click();
 await dialog.getByRole('button',{name:'Fortsätt som Testanvändare'}).click();await dialog.waitFor({state:'hidden'});
 assert.match(await page.locator('.grit-row').first().innerText(),/Osparad ändring/);
 // All routes must fit smaller desktop windows, including charts and overlays.
 for(const width of [1024,800,640,390,320]){
  await page.setViewportSize({width,height:900});
  for(const path of ['/','/report','/timeline','/settings']){
   await page.goto(base+'?review#'+path);
   await page.locator('.workspace-heading h1').waitFor();await page.waitForTimeout(150);
   const overflow=await page.evaluate(()=>{const main=document.querySelector('.app-shell-main-scroll');return {body:document.body.scrollWidth-innerWidth,main:main.scrollWidth-main.clientWidth};});
   if(overflow.main>1){await page.screenshot({path:out+'/overflow.png'});console.log(await page.evaluate(()=>{const main=document.querySelector('.app-shell-main-scroll');const right=main.getBoundingClientRect().right;return [...main.querySelectorAll('*')].filter(e=>e.getBoundingClientRect().right>right+1).slice(0,12).map(e=>({tag:e.tagName,cls:e.className,text:e.textContent.slice(0,60),width:e.getBoundingClientRect().width}));}));}
   assert.ok(overflow.body<=1 && overflow.main<=1,`${path} at ${width}px overflows: ${JSON.stringify(overflow)}`);
  }
 }
 await page.goto(base);await page.getByText('Från aktivitet till tidrapport').waitFor();
 await page.screenshot({path:out+'/06-narrow.png'});
 // Missing credentials and an empty synced directory get actionable empty states.
 await page.setViewportSize({width:1024,height:800});
 await page.evaluate(()=>{const original=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(cmd,args)=>{const result=await original(cmd,args);return cmd==='grit_status'?{...result,users:[],credential_saved:false}:result;};window.dispatchEvent(new Event('grit-changed'));});
 await page.getByRole('button',{name:'Välj Spiris-användare',exact:true}).click();
 await dialog.getByRole('button',{name:'Öppna inställningar'}).waitFor();
 await page.keyboard.press('Escape');
 assert.deepEqual(errors,[]);
 console.log('PASS dashboard: themes, real-state charts, provider logos, user selection/failure/dirty guard, routes and responsive layout');
} catch(error) { await page.screenshot({path:out+'/failure.png'});console.error({url:page.url(),errors,body:(await page.locator('body').innerText()).slice(0,2500)});throw error; } finally { await browser.close(); }
