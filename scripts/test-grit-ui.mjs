import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {chromium} from 'playwright';
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH || '/usr/bin/chromium',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1360,height:1000}});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.addInitScript(()=>{
 const d=new Date();d.setDate(d.getDate()-1);d.setHours(9,0,0,0);const start=Math.floor(d.getTime()/1000);const date=`${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
 const settings={archive_dir:'/mnt/archive/grit',provider:'codex',model:'gpt-6.2-sol',user_id:1,article_id:3,activity_id:2,time_code_id:4,billable:false,timeout_seconds:600};
 let report={date,revision:'review-1',status:'draft',summary:'Arbetade med ORBIT: förbättrade återförsök vid uppladdning och verifierade felhanteringen.',rows:[{id:'row-1',project_id:42,article_id:3,activity_id:2,start,end:start+3600,minutes:60,description:'SAPPHIRE-729: förbättrade återförsök och verifierade felhantering.',evidence_ids:[1,2,3],needs_review:false,remote_id:null}],error:null};
 const status={capture:{interval_seconds:300,display_mode:'active_window',jpeg_quality:85,max_image_width:2560,width_mode:'fixed'},monitors:[{name:'Skärm 1',width:2560,height:1440,x:0,y:0,scale_factor:1},{name:'Skärm 2',width:1920,height:1080,x:2560,y:0,scale_factor:1}],settings,local_dir:'/home/example/.local/share/grit-work',yesterday:date,pending_dates:[],projects:[{id:42,name:'ORBIT',number:'42',active:true}],users:[{id:1,name:'Testanvändare'}],articles:[{id:3,name:'Arbetstid'}],activities:[{id:2,name:'Utveckling'}],time_codes:[{id:4,name:'Normal arbetstid'}],credential_saved:true,report,last_error:null};
 window.testCalls=[];
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>0,convertFileSrc:p=>p,invoke:async(cmd,args)=>{
  window.testCalls.push(cmd);
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
await page.screenshot({path:join(tmpdir(),'grit-report.png'),fullPage:true});
await page.getByRole('button',{name:'Godkänn & skicka till Spiris',exact:true}).click();
if(await page.evaluate(()=>window.testCalls.includes('grit_submit')))throw Error('Submission before confirmation');
await page.getByRole('heading',{name:'Bekräfta överföring till Spiris'}).waitFor();
await page.getByRole('button',{name:'Bekräfta & skicka',exact:true}).click();
await page.waitForFunction(()=>window.testCalls.includes('grit_submit'));
await page.getByRole('button',{name:'Inställningar',exact:true}).click();
await page.getByRole('spinbutton',{name:'Skärmbildsintervall i sekunder'}).fill('60');
await page.getByLabel('Skärmar att samla in').selectOption('all');
await page.getByRole('button',{name:'Spara insamling',exact:true}).click();
await page.getByText('Insamlingen är uppdaterad.',{exact:false}).waitFor();
await page.getByRole('button',{name:'Tidrapport',exact:true}).click();
await page.getByRole('button',{name:'Inställningar',exact:true}).click();
if(await page.getByRole('spinbutton',{name:'Skärmbildsintervall i sekunder'}).inputValue() !== '60') throw Error('Capture settings did not survive navigation');
await page.screenshot({path:join(tmpdir(),'grit-settings.png'),fullPage:true});
if(errors.length)throw Error(errors.join('\n'));
console.log(JSON.stringify({confirmationGate:'passed',pageErrors:errors}));
await browser.close();
