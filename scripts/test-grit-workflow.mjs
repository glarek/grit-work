import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {chromium} from 'playwright';
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH || '/usr/bin/chromium',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1360,height:1000}});
page.setDefaultTimeout(10000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(()=>{
 const d=new Date();d.setDate(d.getDate()-1);d.setHours(9,0,0,0);const start=Math.floor(d.getTime()/1000);const date=`${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
 const settings={archive_dir:'/mnt/archive/grit',provider:'codex',model:'gpt-6.2-sol',user_id:1,article_id:3,activity_id:2,time_code_id:4,billable:false,timeout_seconds:600};
 let report={date,revision:'review-1',status:'draft',summary:'Arbetade med ORBIT: förbättrade återförsök vid uppladdning och verifierade felhanteringen.',rows:[{id:'row-1',project_id:42,article_id:3,activity_id:2,start,end:start+3600,minutes:60,description:'SAPPHIRE-729: förbättrade återförsök och verifierade felhantering.',evidence_ids:[1,2,3],needs_review:false,remote_id:null}],error:null};
 const status={capture:{interval_seconds:300,display_mode:'active_window',jpeg_quality:85,max_image_width:2560,width_mode:'fixed'},monitors:[{name:'Skärm 1',width:2560,height:1440,x:0,y:0,scale_factor:1},{name:'Skärm 2',width:1920,height:1080,x:2560,y:0,scale_factor:1}],settings,local_dir:'/home/example/.local/share/grit-work',yesterday:date,pending_dates:[],projects:[{id:42,name:'ORBIT',number:'42',active:true}],users:[{id:1,name:'Testanvändare'}],articles:[{id:3,name:'Arbetstid'}],activities:[{id:2,name:'Utveckling'}],time_codes:[{id:4,name:'Normal arbetstid'}],credential_saved:true,report,last_error:null};
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

const testDay=new Date();testDay.setDate(testDay.getDate()-3);
const originalDate=`${testDay.getFullYear()}-${String(testDay.getMonth()+1).padStart(2,'0')}-${String(testDay.getDate()).padStart(2,'0')}`;
await page.locator('.grit-work input[type="date"]').fill(originalDate);
await page.locator('.grit-work input[type="date"]').dispatchEvent('change');
await page.getByRole('navigation',{name:'Arbetsflöde för vald dag'}).getByRole('link',{name:'Tidslinje',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(c=>c.cmd==='get_timeline'));
if(!await page.locator('.grit-workflow').innerText().then(t=>t.includes(originalDate)))throw Error('Timeline lost selected date');
await page.getByRole('navigation',{name:'Arbetsflöde för vald dag'}).getByRole('link',{name:'Översikt',exact:true}).click();
await page.waitForFunction(()=>window.testCallArgs.some(c=>c.cmd==='get_overview_stats'));
await page.getByRole('navigation',{name:'Arbetsflöde för vald dag'}).getByRole('link',{name:'Analysera',exact:true}).click();
await page.getByRole('heading',{name:'Tidsanalys',exact:true}).waitFor();
if(await page.getByLabel('Arbetsdag för analys').inputValue() !== originalDate)throw Error('Analysis lost selected date');
await page.getByRole('textbox',{name:'Fråga om arbetsdagen'}).fill('Vilka projekt arbetade jag med?');
await page.getByRole('button',{name:'Starta analys',exact:true}).click();
await page.getByText('Underlaget visar 60 minuter arbete med ORBIT.',{exact:false}).waitFor();
if(await page.evaluate(()=>window.testCalls.some(c=>['grit_submit','generate_report','ask_agent'].includes(c))))throw Error('Analysis invoked legacy AI or submission');
await page.evaluate(()=>window.analysisDelay=3500);
await page.getByRole('textbox',{name:'Fråga om arbetsdagen'}).fill('Kontrollera luckorna i underlaget.');
await page.getByRole('button',{name:'Starta analys',exact:true}).click();
await page.getByRole('navigation',{name:'Arbetsflöde för vald dag'}).getByRole('link',{name:'Tidslinje',exact:true}).click();
await page.getByRole('navigation',{name:'Arbetsflöde för vald dag'}).getByRole('link',{name:'Analysera',exact:true}).click();
await page.getByText('Bakgrundsanalys pågår för',{exact:false}).waitFor();
await page.getByRole('heading',{name:'Kontrollera luckorna i underlaget.',exact:true}).waitFor({timeout:12000});
await page.locator('a[href="#/settings"]').first().click();
await page.waitForTimeout(500);
await page.getByRole('heading',{name:'Skärmbilder & lagring',exact:true}).waitFor();
await page.getByRole('textbox',{name:'Modell',exact:true}).fill('custom-model-draft');
await page.getByRole('spinbutton',{name:'Skärmbildsintervall i sekunder'}).fill('90');
await page.getByRole('button',{name:'Spara insamling',exact:true}).click();
await page.getByText('Insamlingen är uppdaterad.',{exact:false}).waitFor();
if(await page.getByRole('textbox',{name:'Modell',exact:true}).inputValue() !== 'custom-model-draft')throw Error('Capture save erased unsaved CLI settings');
await page.locator('a[href="#/"]').first().click();
if(await page.locator('.grit-work input[type="date"]').inputValue() !== originalDate)throw Error('Report reset shared date');
await page.getByRole('button',{name:'Inställningar',exact:true}).click();
await page.getByRole('spinbutton',{name:'Skärmbildsintervall i sekunder'}).waitFor();
if(await page.getByRole('spinbutton',{name:'Skärmbildsintervall i sekunder'}).inputValue() !== '90')throw Error('Settings diverged across routes');
if(errors.length)throw Error(errors.join('\n'));
console.log(JSON.stringify({sharedDate:'passed',sharedCaptureSettings:'passed',unsavedSettingsPreserved:'passed',analysisDoesNotSubmit:'passed',backgroundAnalysisResume:'passed',pageErrors:errors}));
await browser.close();
