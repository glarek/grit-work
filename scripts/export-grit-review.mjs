import {readFileSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {chromium} from 'playwright';
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH || '/usr/bin/chromium',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1360,height:1150}});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(()=>{
 const d=new Date();d.setDate(d.getDate()-1);d.setHours(9,0,0,0);const start=Math.floor(d.getTime()/1000);const date=`${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
 const settings={archive_dir:'Z:\\Grit Work Archive',provider:'codex',model:'gpt-6.2-sol',user_id:1,article_id:3,activity_id:2,time_code_id:4,billable:false,timeout_seconds:600,archive_retention_days:90,ai_consent:true};
 let report={date,revision:'review-1',status:'draft',summary:'Arbetade med ORBIT: förbättrade återförsök vid uppladdning och verifierade felhanteringen.',rows:[{id:'row-1',project_id:42,article_id:3,activity_id:2,start,end:start+2400,minutes:40,description:'SAPPHIRE-729: förbättrade återförsök och verifierade felhantering.',evidence_ids:[1,2,3],needs_review:false,remote_id:null}],error:null};
 const status={capture:{interval_seconds:300,display_mode:'active_window',jpeg_quality:85,max_image_width:2560,width_mode:'fixed'},monitors:[{name:'Skärm 1',width:2560,height:1440,x:0,y:0,scale_factor:1},{name:'Skärm 2',width:1920,height:1080,x:2560,y:0,scale_factor:1}],settings,local_dir:'C:\\Users\\Example\\AppData\\Local\\grit-work',yesterday:date,pending_dates:[],projects:[{id:42,name:'ORBIT',number:'42',active:true}],users:[{id:1,name:'Testanvändare'}],articles:[{id:3,name:'Arbetstid'}],activities:[{id:2,name:'Utveckling'}],time_codes:[{id:4,name:'Normal arbetstid'}],credential_saved:true,report,last_error:null};
 let analysisHistory=[]; let analysisJob=null;
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 window.testCalls=[]; window.testCallArgs=[];
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>0,convertFileSrc:p=>p,invoke:async(cmd,args)=>{
  window.testCalls.push(cmd); window.testCallArgs.push({cmd,args});
  if(cmd==='grit_day_context')return {source:'archive',activities:3,minutes:60,report_status:report.status,projects:[{id:42,name:'ORBIT',minutes:60,needs_review:false}]};
  if(cmd==='grit_get_analysis')return analysisHistory;
  if(cmd==='grit_analysis_job')return analysisJob;
  if(cmd==='grit_analyze'){analysisJob=args.date; await new Promise(resolve=>setTimeout(resolve,window.analysisDelay||0)); analysisHistory=[...analysisHistory,{question:args.question,answer:'Underlaget visar 60 minuter arbete med ORBIT. Kontrollera projektfördelningen mot tidslinjen före rapportering. Underlag: #1, #2, #3.',provider:settings.provider,model:settings.model,created_at:start+3600}];analysisJob=null;return analysisHistory;}
  if(cmd==='grit_save_settings'){Object.assign(settings,args.settings);return null;}
  if(cmd==='get_timeline')return [{id:1,timestamp:start+3600,duration:3600,app_name:'Editor',window_title:'ORBIT · SAPPHIRE-729',screenshot_path:'',ocr_text:null,category:'work',browser_url:null,executable_path:null,semantic_category:null,semantic_confidence:null}];
  if(['get_hourly_summaries','get_categories','get_custom_semantic_categories','get_semantic_categories','get_hourly_app_breakdown'].includes(cmd))return [];
  if(['get_today_stats','get_daily_stats','get_overview_stats'].includes(cmd))return {total_duration:3600,screenshot_count:3,app_usage:[{app_name:'Editor',duration:3600,count:3}],category_usage:[{category:'work',duration:3600}],browser_duration:0,domain_usage:[],domain_total_count:0,browser_usage:[],work_time_duration:3600,overtime_duration:0,hourly_activity_distribution:[]};
  if(cmd==='get_range_daily_totals')return [{date,total_duration:3600}];
  if(cmd==='get_overview_domains')return {total_count:0,domains:[]};
  if(cmd==='grit_status')return {...status,report};
  if(cmd==='grit_get_report')return report;
  if(cmd==='grit_save_capture_settings'){status.capture={...args.settings};return null;}
  if(cmd==='grit_save_draft'){report={...args.report,revision:'review-2'};return report;}
  if(cmd==='grit_submit'){report={...report,status:'uploaded',rows:report.rows.map(r=>({...r,remote_id:123}))};return report;}
  if(cmd==='get_config')return {locale:'en',theme:'light',ui_visual_style:'a',storage:{screenshots_enabled:true},lightweight_mode:false};
  if(cmd==='get_recording_state')return [true,false];
  if(cmd==='plugin:event|listen')return 0;
  if(cmd==='get_app_version')return '0.1.0';
  if(cmd==='should_check_updates')return false;
  return null;
 }};
});
await page.goto(process.env.GRIT_TEST_URL || 'http://127.0.0.1:5182/');
await page.getByRole('button',{name:'Godkänn & skicka till Spiris',exact:true}).waitFor({timeout:20000});
await page.evaluate(async()=>{
 await document.fonts.load('500 16px "Figtree Variable"');
 await document.fonts.load('500 16px "JetBrains Mono Variable"');
 if(!document.fonts.check('500 16px "Figtree Variable"') || !document.fonts.check('500 16px "JetBrains Mono Variable"')) throw Error('Bundled fonts not loaded');
});

const out = 'all-artifacts/interface-review';
const shots=[];
async function shot(name, locator=page){const path=`${out}/${name}.png`;await locator.screenshot({path});shots.push(path);}
await shot('01-oversikt');
await shot('02-rapport',page.locator('.grit-row').first());
await page.getByRole('button',{name:'Godkänn & skicka till Spiris',exact:true}).click();
await page.getByRole('heading',{name:'Bekräfta överföring till Spiris'}).waitFor();
await shot('05-bekraftelse',page.getByRole('dialog').locator(':scope > div').first());
await page.getByRole('button',{name:'Avbryt',exact:true}).click();
await page.getByRole('button',{name:'Inställningar',exact:true}).click();
await page.getByLabel('Skärmar att samla in').selectOption('all');
await page.locator('.grit-capture summary').filter({hasText:'Bildinställningar'}).click();
await page.locator('.grit-capture summary').filter({hasText:'Skärmar och beräkning'}).click();
await shot('03-insamling',page.locator('.grit-capture'));
await shot('04-generering',page.locator('section').filter({has:page.getByRole('heading',{name:'AI & rapportering',exact:true})}).last());
await shot('04-spiris',page.locator('section').filter({has:page.getByRole('heading',{name:'Anslutning till Spiris',exact:true})}).last());
await page.getByRole('button',{name:'Tidrapport',exact:true}).click();
await page.evaluate(()=>document.documentElement.classList.add('dark'));
await page.waitForTimeout(500);
await shot('06-morkt');
await page.evaluate(()=>document.documentElement.classList.remove('dark'));
await page.getByRole('navigation',{name:'Arbetsflöde för vald dag'}).getByRole('link',{name:'Översikt',exact:true}).click();
await page.waitForFunction(()=>window.testCalls.includes('get_overview_stats'));
await page.waitForTimeout(400);
await shot('07-oversiktsanalys');
await page.getByRole('navigation',{name:'Arbetsflöde för vald dag'}).getByRole('link',{name:'Tidslinje',exact:true}).click();
await page.waitForFunction(()=>window.testCalls.includes('get_timeline'));
await page.waitForTimeout(300);
await shot('08-tidslinje');
await page.getByRole('navigation',{name:'Arbetsflöde för vald dag'}).getByRole('link',{name:'Analysera',exact:true}).click();
await page.getByRole('textbox',{name:'Fråga om arbetsdagen'}).fill('Vilka projekt arbetade jag med?');
await page.getByRole('button',{name:'Starta analys',exact:true}).click();
await page.getByText('Underlaget visar 60 minuter arbete med ORBIT.',{exact:false}).waitFor();
await shot('09-tidsanalys');
if(errors.length)throw Error(errors.join('\n'));
if(await page.evaluate(()=>window.testCalls.includes('grit_submit')))throw Error('Unexpected mock submission');
const img=name=>`data:image/png;base64,${readFileSync(`${out}/${name}.png`).toString('base64')}`;
const font=path=>`data:font/woff2;base64,${readFileSync(path).toString('base64')}`;
const pages=[
 ['Överblick','Tidrapporten samlar dagens förslag, projekt, tid och granskningsstatus. Figtree används för text och JetBrains Mono för siffror och tider.',['01-oversikt']],
 ['Granska en rapportrad','Kontrollera projekt, aktivitet, beskrivning och tider innan överföring. Är informationen tillräcklig för att bedöma arbetet?',['02-rapport']],
 ['Skärmbilder & lagring','Valbart intervall 30 sekunder–60 minuter, aktiv skärm eller alla skärmar. Uppskattningen visar bildlagring per dag och månad. Två skärmar är simulerade i detta exempel.',['03-insamling']],
 ['Generering & anslutning','Välj lokal AI-CLI, långtidsarkiv och Spiris-standarder. Anslutningsuppgifter fylls i i appen och lagras i datorns nyckellager.',['04-generering','04-spiris']],
 ['Bekräfta överföringen','Ett separat bekräftelsesteg före överföring. Underlaget här är syntetiskt och ingen verklig tid har skickats.',['05-bekraftelse']],
 ['Mörkt tema','Samma arbetsflöde med mörka ytor, tydliga kontraster och markerad primär åtgärd. Granska läsbarhet och hierarki.',['06-morkt']],
 ['Översikt med gemensam arbetsdag','Originalets aktivitetsstatistik och diagram kopplas till samma dag, underlag och rapportstatus som tidrapporten.',['07-oversiktsanalys']],
 ['Tidslinje och underlag','Granska aktivitet och bilder innan projektfördelningen fastställs. Verifierade arkiv visas även efter lokal rensning.',['08-tidslinje']],
 ['Tidsanalys via samma CLI','Frågor använder samma agentval och underlag som rapportgenereringen. Analysen sparar svar för dagen men ändrar inga rapporter och skickar ingen tid till Spiris.',['09-tidsanalys']]
];
const html=`<!doctype html><html lang="sv"><meta charset="utf-8"><title>Grit Work – interfacegranskning</title><style>
@font-face{font-family:Figtree;src:url(${font('node_modules/@fontsource-variable/figtree/files/figtree-latin-wght-normal.woff2')})}
@font-face{font-family:Mono;src:url(${font('node_modules/@fontsource-variable/jetbrains-mono/files/jetbrains-mono-latin-wght-normal.woff2')})}
@page{size:A4 landscape;margin:12mm}*{box-sizing:border-box}body{margin:0;font-family:Figtree,sans-serif;color:#172033}section{height:185mm;break-after:page;display:flex;flex-direction:column}section:last-child{break-after:auto}.eyebrow{font:11px Mono;color:#626d80;margin-bottom:7px}h1{font-size:25px;margin:0 0 7px;font-weight:650}p{font-size:12px;line-height:1.5;margin:0 0 12px;max-width:950px}.images{flex:1;min-height:0;display:flex;gap:14px;align-items:flex-start;justify-content:center}.images img{max-width:100%;max-height:100%;object-fit:contain;object-position:top;border:1px solid #dbe1eb;border-radius:5px}.images.dual img{max-width:48%;height:100%;border:0}footer{font:9px Mono;color:#6c7586;display:flex;justify-content:space-between;margin-top:10px}
</style>${pages.map(([title,caption,images],i)=>`<section><div class="eyebrow">GRIT WORK / INTERFACEGRANSKNING</div><h1>${title}</h1><p>${caption}</p><div class="images ${images.length>1?'dual':''}">${images.map(n=>`<img src="${img(n)}">`).join('')}</div><footer><span>Syntetiskt testunderlag · 2026-11-18 · 1.1.4-rc.2</span><span>${i+1} / ${pages.length}</span></footer></section>`).join('')}</html>`;
writeFileSync(`${out}/review.html`,html);
const print=await browser.newPage();await print.setContent(html,{waitUntil:'load'});await print.evaluate(()=>document.fonts.ready);
await print.pdf({path:`${out}/Grit-Work-interfacegranskning.pdf`,format:'A4',landscape:true,printBackground:true,preferCSSPageSize:true});
console.log(JSON.stringify({pdf:`${out}/Grit-Work-interfacegranskning.pdf`,screenshots:shots.length,pageErrors:errors}));
await browser.close();
