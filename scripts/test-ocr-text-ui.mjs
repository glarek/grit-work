import assert from 'node:assert/strict';
import {readFile,mkdir} from 'node:fs/promises';
import {chromium} from 'playwright';
import {installReviewFixture} from './grit-review-fixture.mjs';
const corpus=JSON.parse(await readFile(new URL('../tests/fixtures/ocr-text-corpus.json',import.meta.url),'utf8'));
corpus.push({id:'long-unbroken-identifier',text:'Å漢🧑'.repeat(400)});
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH||'/usr/bin/chromium',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1024,height:800},reducedMotion:'reduce'});
page.setDefaultTimeout(15000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(installReviewFixture());
await page.addInitScript(corpus=>{
 const index=Number(new URLSearchParams(location.search).get('case'))||0;
 const text=corpus[index].text;
 const original=window.__TAURI_INTERNALS__.invoke;
 const reportWithText=report=>report?{...report,summary:text,rows:report.rows.map(row=>({...row,description:text}))}:report;
 window.__TAURI_INTERNALS__.invoke=async(cmd,args)=>{
  const result=await original(cmd,args);
  if(cmd==='grit_status')return {...result,report:reportWithText(result.report)};
  if(cmd==='grit_get_report')return reportWithText(result);
  if(cmd==='get_timeline')return result.map(row=>({...row,window_title:text,ocr_text:text}));
  return result;
 };
},corpus);
const base=process.env.GRIT_TEST_URL||'http://127.0.0.1:5183/';
await mkdir('all-artifacts/ocr-robustness',{recursive:true});
try {
 for(let index=0;index<corpus.length;index++){
  const {id,text}=corpus[index];
  await page.goto(`${base}?case=${index}#/report`);
  const row=page.locator('.grit-row').first();await row.click();
  const field=page.getByRole('textbox',{name:'Dagbok',exact:true});
  const normalized=text.replace(/\r\n?/g,'\n');
  assert.equal(await field.inputValue(),normalized,`${id}: editor must preserve Unicode`);
  await field.fill(text);
  await page.getByRole('button',{name:'Spara rad',exact:true}).click();
  await page.getByRole('button',{name:'Spara utkast',exact:true}).click();
  const saved=await page.evaluate(()=>window.testCallArgs.filter(call=>call.cmd==='grit_save_draft').at(-1));
  assert.equal(saved.args.report.rows[0].description,normalized,`${id}: saved draft text`);
  assert.equal(await page.evaluate(()=>window.ocrExecuted),undefined,`${id}: HTML must remain text`);
  assert.equal(await page.locator('.grit-work script,.grit-work img[onerror]').count(),0);
  assert.equal(await page.evaluate(()=>window.testCalls.includes('grit_submit')),false);
  const overflow=await page.locator('.app-shell-main-scroll').evaluate(e=>e.scrollWidth-e.clientWidth);
  assert.ok(overflow<=1,`${id}: report overflow ${overflow}`);
  if(id==='html')await page.screenshot({path:'all-artifacts/ocr-robustness/literal-html.png'});
  await page.getByRole('link',{name:'Tidslinje',exact:true}).click();
  await page.locator('.timeline-entry-title').first().waitFor();
  assert.equal(await page.evaluate(()=>window.ocrExecuted),undefined,`${id}: timeline HTML must remain text`);
  assert.equal(await page.locator('.timeline-entry script,.timeline-entry img[onerror]').count(),0);
 }
 assert.deepEqual(errors,[]);
 console.log(`PASS ${corpus.length} OCR text cases: Unicode editor/save round-trip, literal HTML, no script execution, no live submission, bounded layout`);
} catch(error){await page.screenshot({path:'all-artifacts/ocr-robustness/ui-failure.png'});console.error(errors);throw error;} finally {await browser.close();}
