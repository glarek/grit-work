export function installReviewFixture() {
 return ()=>{
 if (!location.search.includes('onboarding')) localStorage.setItem('grit-onboarding-v1','deferred');
 const d=new Date();d.setDate(d.getDate()-1);d.setHours(9,0,0,0);const start=Math.floor(d.getTime()/1000);const date=`${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
 const settings={archive_dir:'Z:\\GRIT Work Arkiv',provider:'codex',model:'gpt-6.2-sol',user_id:1,article_id:3,activity_id:2,time_code_id:4,billable:false,timeout_seconds:600,archive_retention_days:90,ai_consent:!location.search.includes('onboarding'),use_report_history:false,image_interpretation:{mode:'off',interval_seconds:300,provider:'codex',model:'gpt-6.2-sol'}};
 let report={date,revision:'review-1',status:'draft',summary:'Arbetade med ORBIT: förbättrade återförsök vid uppladdning och verifierade felhanteringen.',rows:[{id:'row-1',project_id:42,article_id:0,activity_id:0,time_code_id:0,start,end:start+3600,minutes:60,description:'SAPPHIRE-729: förbättrade återförsök och verifierade felhantering.',evidence_ids:[1,2,3],needs_review:false,remote_id:null}],error:null};
 const status={capture:{interval_seconds:300,display_mode:'active_window',jpeg_quality:85,max_image_width:0,width_mode:'fixed'},monitors:[{name:'Skärm 1',width:2560,height:1440,x:0,y:0,scale_factor:1},{name:'Skärm 2',width:1920,height:1080,x:2560,y:0,scale_factor:1}],settings,local_dir:'C:\\Users\\Example\\AppData\\Local\\grit-work',yesterday:date,pending_dates:[],projects:[{id:42,name:'ORBIT',number:'42',active:true}],users:[{id:1,name:'Testanvändare'}],articles:[{id:3,name:'Arbetstid'}],activities:[{id:2,name:'Utveckling'}],time_codes:[{id:4,name:'Normal arbetstid'}],credential_saved:!location.search.includes('onboarding'),report,last_error:null};
 if(location.search.includes('review')) {
  status.projects.push({id:43,name:'LINDEN',number:'1043',active:true},{id:44,name:'Interntid',number:'9000',active:true});
  const descriptions=['Samordnat elprojektering och uppdaterat ritningsunderlag efter avstämning.','Kontrollerat kabeldimensionering och dokumenterat ändringar i gruppförteckningen.','Tagit fram underlag inför byggmöte och stämt av öppna frågor.','Gått igenom granskningssynpunkter och justerat bygghandlingar.','Projekteringsmöte med konstruktör. Följt upp gränsdragningar.','Uppdaterat interna mallar för tekniska beskrivningar.','Reviderat belysningsplan och samordnat placeringar med arkitekt.','Dokumenterat dagens beslut och förberett nästa granskningsleverans.'];
  report={...report,summary:'Projektering och samordning i ORBIT och LINDEN. Intern mallförbättring.',rows:descriptions.map((description,i)=>({...report.rows[0],id:'review-'+i,project_id:[42,42,43,43,42,44,43,42][i],start:start+i*1800,end:start+(i+1)*1800,minutes:30,description,needs_review:i===2}))};
 }
 const interpretationStatusByDate={};
 function getInterpStatus(targetDate){
  const mode=settings.image_interpretation?.mode||'off';
  const interval=settings.image_interpretation?.interval_seconds||300;
  const requiresConsent=mode==='ai' && !settings.ai_consent;
  if(!interpretationStatusByDate[targetDate]){
   interpretationStatusByDate[targetDate]={date:targetDate,file:`image-interpretations/${targetDate}.json`,completed:0,pending:2,failed:0,disabled:0,skipped:0,running:false,mode,next_run_at:mode==='off'||requiresConsent?null:Math.floor(Date.now()/1000)+interval,requires_consent:requiresConsent,last_error:null};
  }
  const base={...interpretationStatusByDate[targetDate],mode,requires_consent:requiresConsent};
  const override=typeof window.testInterpretationStatus==='function'?window.testInterpretationStatus(targetDate,base):window.testInterpretationStatus;
  return override && typeof override==='object'?{...base,...override}:base;
 }
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 window.testCalls=[]; window.testCallArgs=[];
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>0,convertFileSrc:p=>p,invoke:async(cmd,args)=>{
  window.testCalls.push(cmd); window.testCallArgs.push({cmd,args});
  if(cmd==='grit_connection_status')return window.testConnectionHealth || {spiris:{state:'ok',checked_at:Date.now()/1000,message:'Autentisering och läsbehörigheter verifierade. Skrivbehörighet är inte testad.'},ai:{state:'ok',checked_at:Date.now()/1000,message:'Rapportmodellen svarade på testet.'},ai_image:null};
  if(cmd==='grit_day_context')return {source:'archive',activities:3,minutes:60,report_status:report.status,projects:[{id:42,name:'ORBIT',minutes:60,needs_review:false}]};
  if(cmd==='grit_interpretation_status')return getInterpStatus(args.date);
  if(cmd==='grit_interpret_images'){
   const targetDate=args.date;
   const mode=settings.image_interpretation?.mode||'off';
   const interval=settings.image_interpretation?.interval_seconds||300;
   if(mode==='ai' && !settings.ai_consent){
    const err='AI-samtycke krävs innan AI-bildtolkning körs.';
    interpretationStatusByDate[targetDate]={date:targetDate,file:`image-interpretations/${targetDate}.json`,completed:0,pending:0,failed:2,disabled:0,skipped:0,running:false,mode,next_run_at:null,requires_consent:true,last_error:err};
    throw new Error(err);
   }
   interpretationStatusByDate[targetDate]=mode==='off'
    ? {date:targetDate,file:`image-interpretations/${targetDate}.json`,completed:0,pending:0,failed:0,disabled:2,skipped:0,running:false,mode,next_run_at:null,requires_consent:false,last_error:null}
    : {date:targetDate,file:`image-interpretations/${targetDate}.json`,completed:2,pending:0,failed:0,disabled:0,skipped:0,running:false,mode,next_run_at:Math.floor(Date.now()/1000)+interval,requires_consent:false,last_error:null};
   return getInterpStatus(targetDate);
  }
  if(cmd==='grit_cancel')return null;
  if(cmd==='grit_save_settings'){Object.assign(settings,args.settings,{image_interpretation:{...(args.settings?.image_interpretation??settings.image_interpretation)}});return null;}
  if(cmd==='get_timeline')return [{id:1,timestamp:start+3600,duration:3600,app_name:'Editor',window_title:'ORBIT · SAPPHIRE-729',screenshot_path:'',ocr_text:null,category:'development',browser_url:null,executable_path:null,semantic_category:null,semantic_confidence:null}];
  if(cmd==='get_categories')return [{key:'development',name:'Utveckling',color:'#6366f1',is_system:true}];
  if(['get_hourly_summaries','get_categories','get_custom_semantic_categories','get_semantic_categories','get_hourly_app_breakdown'].includes(cmd))return [];
  if(['get_today_stats','get_daily_stats','get_overview_stats'].includes(cmd))return {total_duration:3600,screenshot_count:3,app_usage:[{app_name:'Editor',duration:3600,count:3}],category_usage:[{category:'development',duration:3600}],browser_duration:0,domain_usage:[],domain_total_count:0,browser_usage:[],work_time_duration:3600,overtime_duration:0,hourly_activity_distribution:[]};
  if(cmd==='get_range_daily_totals')return [{date,total_duration:3600}];
  if(cmd==='get_overview_domains')return {total_count:0,domains:[]};
  if(cmd==='grit_status')return {...status,report};
  if(cmd==='grit_get_report')return {...report,date:args.date};
  if(cmd==='grit_generate'){
   if(window.testGenerateFailure)throw new Error('CLI-inloggning saknas. Logga in och försök igen.');
   report={...report,date:args.date,revision:'generated-'+Date.now(),basis:{created_at:Date.now()/1000,activities:3,active_seconds:3600,images_completed:2,images_with_content:1,images_pending:1,images_failed:0}};return report;
  }
  if(cmd==='grit_reporting_context')return {date:args.date,user:status.users[0],checked_at:Date.now()/1000,projects:report.rows.map(row=>({project_id:row.project_id,member:!window.testNoMembership,active:true,activities:status.activities,time_codes:status.time_codes,articles:status.articles,restriction_source:'active_registries_no_project_restrictions_exposed'})),existing_rows:window.testDuplicates?[{id:200,date:args.date,project:status.projects[0],activity:status.activities[0],time_code:status.time_codes[0],article:status.articles[0],hours:1,comment:'Redan rapporterat arbete'}]:[],previous_rows:[{id:100,date:'2026-10-20',project:status.projects[0],activity:status.activities[0],time_code:status.time_codes[0],article:status.articles[0],hours:1.5,comment:'Uppdaterat bygghandlingar och samordnat ändringar.'}]};
  if(cmd==='grit_test_ai')return {target:args.target,ok:!window.testAiFailure,message:window.testAiFailure?'CLI-inloggning saknas. Logga in och försök igen.':args.target==='image'?'Modellen läste testbilden och lämnade ett giltigt svar.':'Modellen svarade med rätt format.'};
  if(cmd==='grit_test_connection'){const c=(resource)=>({resource,ok:true,tested:true,http_status:200,count:1,message:'Läsåtkomst verifierad'});return {checked_at:Date.now()/1000,authentication:c('authentication'),reads:['Core/Projects','Admin/Users','Admin/Activities','Admin/TimeCodes','Admin/Articles','Core/TimeReports'].map(c),submission_permission:'not_tested'};}
  if(cmd==='grit_save_credentials'){status.credential_saved=true;return null;}
  if(cmd==='grit_save_capture_settings'){status.capture={...args.settings};return null;}
  if(cmd==='grit_save_draft'){report={...args.report,revision:'review-2'};return report;}
  if(cmd==='grit_submit'){report={...report,status:'uploaded',rows:report.rows.map(r=>({...r,remote_id:123}))};return report;}
  if(cmd==='get_config')return {locale:'en',theme:'dark',ui_visual_style:'c',storage:{screenshots_enabled:true},lightweight_mode:false,privacy:{excluded_apps:[],excluded_title_keywords:[],excluded_urls:[]},work_time_segments:[],auto_start:true,auto_start_silent:false};
  if(cmd==='get_runtime_platform')return 'windows';
  if(cmd==='get_running_apps' || cmd==='get_recent_apps')return [];
  if(cmd==='get_recording_state')return [true,false];
  if(cmd==='is_autostart_enabled')return true;
  if(cmd==='plugin:event|listen')return 0;
  if(cmd==='get_app_version' || cmd==='plugin:app|version')return '1.1.4-rc.3';
  if(cmd==='should_check_updates')return false;
  return null;
 }};

 };
}
