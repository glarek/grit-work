import assert from 'node:assert/strict';
import {chromium} from 'playwright';
import {installReviewFixture} from './grit-review-fixture.mjs';
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH || '/usr/bin/chromium',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1360,height:1000}});page.setDefaultTimeout(15000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(installReviewFixture());
await page.goto(process.env.GRIT_TEST_URL || 'http://127.0.0.1:5182/');
await page.getByRole('button',{name:'Rapportera till Spiris',exact:true}).waitFor();
await page.getByRole('button',{name:'Föregående dag',exact:true}).click();
const originalDate=(await page.getByRole('button',{name:'Välj arbetsdag',exact:true}).innerText()).trim();
for(const nav of ['Tidslinje','Översikt']){
 await page.getByRole('link',{name:nav,exact:true}).click();
 await page.getByRole('heading',{name:nav==='Översikt'?'Arbetsöversikt':nav,exact:true}).waitFor();
 assert.match(await page.locator('.grit-workflow').innerText(),new RegExp(originalDate));
}
assert.equal(await page.getByRole('link',{name:'Tidsanalys',exact:true}).count(),0);
await page.getByRole('link',{name:'Inställningar',exact:true}).click();
await page.locator('.settings-tab-rail').getByRole('button',{name:/^AI & rapportering/}).click();
await page.getByRole('button',{name:'Avancerade AI-inställningar',exact:false}).click();
await page.getByRole('textbox',{name:'Modell',exact:true}).fill('custom-model-draft');
const historyChoice=page.getByRole('checkbox',{name:/Använd min skrivstil från Spiris/});
assert.equal(await historyChoice.isChecked(),false);await historyChoice.check();
await page.getByRole('button',{name:'Insamling & lagring',exact:true}).click();
await page.getByRole('spinbutton',{name:'Skärmbildsintervall i sekunder'}).fill('90');
await page.getByRole('button',{name:'Spara insamling',exact:true}).click();
await page.getByText('Insamlingen är uppdaterad.',{exact:false}).waitFor();
await page.locator('.settings-tab-rail').getByRole('button',{name:/^AI & rapportering/}).click();
assert.equal(await page.getByRole('textbox',{name:'Modell',exact:true}).inputValue(),'custom-model-draft');
assert.equal(await historyChoice.isChecked(),true);
await page.getByRole('button',{name:'Spara AI-inställningar',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(call=>call.cmd==='grit_save_settings' && call.args.settings.use_report_history===true));

// Image interpretation settings: off / ocr / ai, dirty gating, consent, and manual run
await page.getByRole('button',{name:'Insamling & lagring',exact:true}).click();
async function assertRowAligned(locators,label){
 const boxes=await Promise.all(locators.map(async l=>{await l.waitFor();return l.boundingBox();}));
 for(const b of boxes){assert.ok(b,`${label}: missing boundingBox`);}
 const base=boxes[0];
 for(const b of boxes.slice(1)){
  assert.ok(Math.abs(b.y-base.y)<=1,`${label}: y mismatch (${b.y} vs ${base.y})`);
  assert.ok(Math.abs(b.height-base.height)<=1,`${label}: height mismatch (${b.height} vs ${base.height})`);
 }
}
await assertRowAligned([page.getByRole('spinbutton',{name:'Skärmbildsintervall i sekunder'}),page.getByRole('combobox',{name:'Skärmar att samla in',exact:true})],'captureRow');
await page.getByRole('button',{name:/Bildinställningar/}).click();
await assertRowAligned([page.locator('#capture-jpeg-quality'),page.getByRole('combobox',{name:'Bildstorlek',exact:true}),page.locator('#capture-max-width')],'imageAdvancedRow');
await assertRowAligned([page.getByRole('combobox',{name:'Tolkningsläge',exact:true}),page.locator('#grit-image-interp-interval')],'imageInterpRow');
await assertRowAligned([page.locator('#grit-archive-dir'),page.getByRole('button',{name:'Välj mapp',exact:true}),page.locator('#grit-archive-retention')],'archiveRow');
await page.getByRole('button',{name:'Skapa tomt underlag',exact:true}).click();
await page.getByText('2 av',{exact:false}).waitFor();

await page.getByRole('combobox',{name:'Tolkningsläge',exact:true}).click();
await page.getByRole('option',{name:'Lokal OCR (Windows)',exact:true}).click();
await page.getByText('Spara ändringarna först',{exact:true}).waitFor();
assert.equal(await page.getByRole('button',{name:'Tolka nu',exact:true}).isDisabled(),true);
await page.getByRole('button',{name:'Spara bildtolkning',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(call=>call.cmd==='grit_save_settings' && call.args.settings.image_interpretation?.mode==='ocr'));
await page.getByRole('button',{name:'Tolka nu',exact:true}).click();
await page.getByText('2 klara',{exact:false}).waitFor();

await page.getByRole('combobox',{name:'Tolkningsläge',exact:true}).click();
await page.getByRole('option',{name:'AI-bildtolkning',exact:true}).click();
await assertRowAligned([page.getByRole('combobox',{name:'AI-leverantör för bildtolkning',exact:true}),page.locator('#grit-image-interp-model')],'imageInterpAiRow');
await page.getByRole('combobox',{name:'AI-leverantör för bildtolkning',exact:true}).click();
await page.getByRole('option',{name:/Grok CLI/}).click();
assert.equal(await page.locator('#grit-image-interp-model').inputValue(),'grok-4.7-build-fast');
await page.getByRole('button',{name:'Spara bildtolkning',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(call=>call.cmd==='grit_save_settings' && call.args.settings.image_interpretation?.mode==='ai' && call.args.settings.image_interpretation?.provider==='grok'));
await page.getByRole('button',{name:'Tolka nu',exact:true}).click();
await page.getByText('2 klara',{exact:false}).waitFor();

await page.getByRole('link',{name:'Tidrapporter',exact:true}).click();
assert.equal((await page.getByRole('button',{name:'Välj arbetsdag',exact:true}).innerText()).trim(),originalDate);
assert.equal(await page.evaluate(()=>window.testCalls.some(c=>['grit_submit','generate_report','ask_agent','grit_analyze'].includes(c))),false);
assert.deepEqual(errors,[]);
console.log(JSON.stringify({sharedDate:'passed',unsavedSettingsPreserved:'passed',styleHistoryOptIn:'passed',imageInterpretationModes:'passed',dirtyRunDisabled:'passed',pageErrors:errors}));
await browser.close();
