import assert from 'node:assert/strict';
import {chromium} from 'playwright';
import {installReviewFixture} from './grit-review-fixture.mjs';
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH || '/usr/bin/chromium',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1360,height:1000}});page.setDefaultTimeout(15000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(installReviewFixture());
await page.goto(process.env.GRIT_TEST_URL || 'http://127.0.0.1:5182/');
await page.getByRole('button',{name:'Rapportera till Spiris',exact:true}).waitFor();
// Today is selectable and a draft can be regenerated without submitting time.
assert.equal(await page.getByRole('button',{name:'Idag',exact:true}).isDisabled(),true);
const generate=page.getByRole('button',{name:'Generera om utkast',exact:true});
assert.equal(await generate.isEnabled(),true);
await generate.click();
await page.getByRole('button',{name:'Avbryt',exact:true}).click();
assert.equal(await page.evaluate(()=>window.testCalls.includes('grit_generate')),false);
await generate.click();
await page.getByRole('button',{name:'Ersätt med nytt utkast',exact:true}).click();
await page.locator('[data-report-basis]').waitFor();
assert.match(await page.locator('[data-report-basis]').innerText(),/1 bilder utan färdig tolkning/);
assert.equal(await page.getByRole('button',{name:'Rapportera till Spiris',exact:true}).isDisabled(),true);
await page.evaluate(()=>window.testGenerateFailure=true);
await generate.click();await page.getByRole('button',{name:'Ersätt med nytt utkast',exact:true}).click();
await page.getByText('CLI-inloggning saknas. Logga in och försök igen.',{exact:true}).waitFor();
assert.equal(await page.locator('.grit-row').count(),1,'Failed regeneration retains previous rows');
await page.evaluate(()=>window.testGenerateFailure=false);
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

await page.getByRole('button',{name:'Testa AI',exact:true}).click();
await page.getByText('Modellen svarade med rätt format.',{exact:true}).waitFor();
await page.evaluate(()=>window.testAiFailure=true);
await page.getByRole('button',{name:'Testa AI',exact:true}).click();
await page.getByText('CLI-inloggning saknas. Logga in och försök igen.',{exact:true}).waitFor();
await page.evaluate(()=>window.testAiFailure=false);

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
assert.equal(await page.locator('#capture-max-width').count(),0);
await page.getByRole('switch',{name:'Begränsa bildbredd',exact:true}).click();
await assertRowAligned([page.locator('#capture-jpeg-quality'),page.locator('#capture-max-width')],'imageAdvancedRow');
await page.getByRole('switch',{name:'Begränsa bildbredd',exact:true}).click();
await page.getByRole('button',{name:'Spara insamling',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(c=>c.cmd==='grit_save_capture_settings' && c.args.settings.max_image_width===0));
await assertRowAligned([page.getByRole('combobox',{name:'Tolkningsläge',exact:true}),page.locator('#grit-image-interp-interval')],'imageInterpRow');
await assertRowAligned([page.locator('#grit-archive-dir'),page.getByRole('button',{name:'Välj mapp',exact:true}),page.locator('#grit-archive-retention')],'archiveRow');
async function assertRowAlignedOrStacked(locators,label){
 const boxes=await Promise.all(locators.map(async l=>{await l.waitFor();return l.boundingBox();}));
 for(const b of boxes){assert.ok(b,`${label}: missing boundingBox`);}
 const base=boxes[0];
 const sameRow=boxes.slice(1).every(b=>Math.abs(b.y-base.y)<=1);
 if(sameRow){
  for(const b of boxes.slice(1)){
   assert.ok(Math.abs(b.height-base.height)<=1,`${label}: height mismatch (${b.height} vs ${base.height})`);
  }
 } else {
  for(let i=1;i<boxes.length;i++){
   assert.ok(boxes[i].y>=boxes[i-1].y+boxes[i-1].height-1,`${label}: stacked overlap at ${i}`);
   assert.ok(Math.abs(boxes[i].width-base.width)<=2,`${label}: stacked width mismatch (${boxes[i].width} vs ${base.width})`);
  }
 }
}
for(const width of [920,780]){
 await page.setViewportSize({width,height:900});
 await assertRowAlignedOrStacked([page.getByRole('spinbutton',{name:'Skärmbildsintervall i sekunder'}),page.getByRole('combobox',{name:'Skärmar att samla in',exact:true})],`captureRow-${width}`);
 await assertRowAlignedOrStacked([page.getByRole('combobox',{name:'Tolkningsläge',exact:true}),page.locator('#grit-image-interp-interval')],`imageInterpRow-${width}`);
}
await page.setViewportSize({width:1360,height:1000});
await page.getByRole('button',{name:'Skapa tomt underlag',exact:true}).click();
await page.getByText('2 av',{exact:false}).waitFor();

await page.getByRole('combobox',{name:'Tolkningsläge',exact:true}).click();
await page.getByRole('option',{name:'Lokal OCR (Windows)',exact:true}).click();
await page.getByText('Spara ändringarna först',{exact:true}).waitFor();
assert.equal(await page.getByRole('button',{name:'Tolka nu',exact:true}).isDisabled(),true);
await page.getByRole('button',{name:'Spara bildtolkning',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(call=>call.cmd==='grit_save_settings' && call.args.settings.image_interpretation?.mode==='ocr'));
await page.getByRole('button',{name:'Tolka nu',exact:true}).click();
await page.getByText('2 behandlade',{exact:false}).waitFor();

await page.getByRole('combobox',{name:'Tolkningsläge',exact:true}).click();
await page.getByRole('option',{name:'AI-bildtolkning',exact:true}).click();
await assertRowAligned([page.getByRole('combobox',{name:'AI-leverantör för bildtolkning',exact:true}),page.locator('#grit-image-interp-model')],'imageInterpAiRow');
await page.getByRole('combobox',{name:'AI-leverantör för bildtolkning',exact:true}).click();
await page.getByRole('option',{name:/Grok CLI/}).click();
assert.equal(await page.locator('#grit-image-interp-model').inputValue(),'grok-4.7-build-fast');
await page.getByRole('button',{name:'Spara bildtolkning',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(call=>call.cmd==='grit_save_settings' && call.args.settings.image_interpretation?.mode==='ai' && call.args.settings.image_interpretation?.provider==='grok'));
await page.getByRole('button',{name:'Tolka nu',exact:true}).click();
await page.getByText('2 behandlade',{exact:false}).waitFor();

await page.getByRole('button',{name:'Testa bildmodell',exact:true}).click();
await page.getByText('Modellen läste testbilden och lämnade ett giltigt svar.',{exact:true}).waitFor();
await page.evaluate(()=>window.testInterpretationStatus={completed:48,with_content:8,no_text:40,sparse:8});
await page.getByText('40 bilder utan läst innehåll, 8 med lite innehåll.',{exact:true}).waitFor();
await page.evaluate(()=>window.testInterpretationStatus=null);

await page.getByRole('link',{name:'Tidrapporter',exact:true}).click();
assert.equal((await page.getByRole('button',{name:'Välj arbetsdag',exact:true}).innerText()).trim(),originalDate);
assert.equal(await page.evaluate(()=>window.testCalls.some(c=>['grit_submit','generate_report','ask_agent','grit_analyze'].includes(c))),false);
// The last half hour can be edited and saved without moving it into the next day.
await page.goto((process.env.GRIT_TEST_URL || 'http://127.0.0.1:5182/')+'?midnight');
await page.getByRole('button',{name:'Föregående dag',exact:true}).click();
await page.locator('.grit-row').first().click();
await page.getByText('Tidsintervall och underlag',{exact:true}).click();
assert.equal(await page.getByRole('textbox',{name:'Slut',exact:true}).inputValue(),'24:00');
await page.getByRole('textbox',{name:'Slut',exact:true}).fill('24:01');
await page.getByRole('button',{name:'Spara rad',exact:true}).click();
await page.getByRole('alert').waitFor();
await page.getByRole('textbox',{name:'Slut',exact:true}).fill('24:00');
await page.getByRole('button',{name:'Spara rad',exact:true}).click();
await page.getByRole('dialog').waitFor({state:'hidden'});
await page.getByRole('button',{name:'Spara utkast',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(c=>c.cmd==='grit_save_draft' && c.args.report.rows[0].end-c.args.report.rows[0].start===1815));
assert.deepEqual(errors,[]);
console.log(JSON.stringify({sharedDate:'passed',unsavedSettingsPreserved:'passed',styleHistoryOptIn:'passed',imageInterpretationModes:'passed',dirtyRunDisabled:'passed',pageErrors:errors}));
await browser.close();
