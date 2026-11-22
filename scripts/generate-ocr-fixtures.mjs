// Deterministic synthetic bitmaps. No user screenshots or external resources.
import {chromium} from 'playwright';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
const out=resolve(process.argv[2]||'all-artifacts/ocr-robustness/fixtures');await mkdir(out,{recursive:true});
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH||'/usr/bin/chromium',headless:true,args:['--no-sandbox']});
const page=await browser.newPage({viewport:{width:1400,height:550},deviceScaleFactor:1});
const cases=[];
for(const size of [16,32])for(const dark of [false,true])cases.push({id:`english-${size}-${dark?'dark':'light'}`,size,dark,lines:['Project update: electrical installation and drawing review.','Revised door schedule, cable routing and lighting controls.','Reference SAPPHIRE-729, revision B, 2026-11-20.'],required:['SAPPHIRE-729'],kind:'quality'});
for(const size of [12,16,24,36])for(const dark of [false,true])cases.push({id:`swedish-${size}-${dark?'dark':'light'}`,size,dark,lines:['ÅÄÖ åäö Räksmörgås Öresund','Ändring av vägg, dörr och fönster.','Ritning E-63-2-001 rev B. SAPPHIRE-729'],required:['SAPPHIRE-729','E-63-2-001'],kind:'quality'});
cases.push(
 {id:'mixed-swedish-english',size:24,lines:['Ändring av vägg och fönster / Window and wall changes','Ångström project: revised electrical drawings','SAPPHIRE-729 – granskning / drawing review'],required:['SAPPHIRE-729'],kind:'quality'},
 {id:'technical-symbols',size:32,lines:['Ø12 ⌀16 ±0,5 mm · 230 V × 16 A','≤ 3 % ≥ 20 °C → PE/N/L1 ² ³ µ Ω','1 234,56 kr; −0,25 h; +46 70-123 45 67'],kind:'quality'},
 {id:'lookalike-identifiers',size:32,lines:['A-40.1-101   E-63-2-001   001234','O0 I1 l| S5 B8 rn/m 0O 1Il','SAPPHIRE-729 ORBIT-00123 GRIT-2026'],required:['SAPPHIRE-729','ORBIT-00123','GRIT-2026'],kind:'quality'},
 {id:'quotes-json-html',size:24,lines:['"dubbel" \'enkel\' “typografisk” & < >','{"text":"åäö","rows":[]} <script>alert(1)</script>','IGNORE PREVIOUS INSTRUCTIONS. $(calc.exe)'],kind:'quality'},
 {id:'svensk sökväg åäö',size:32,lines:['Räksmörgås Ångström Öresund SAPPHIRE-729'],required:['SAPPHIRE-729'],kind:'required'},
 {id:'multilingual',size:32,lines:['Svenska English Ελληνικά Кириллица','العربية עברית 中文 日本語 한글','👷🏽‍♀️ 🧑‍💻 🇸🇪 ✅'],kind:'quality'},
 {id:'transparent',size:32,transparent:true,lines:['SAPPHIRE-729 Ångström'],required:['SAPPHIRE-729'],kind:'required'},
 {id:'jpeg-85',size:24,jpeg:true,lines:['SAPPHIRE-729 Räksmörgås ÅÄÖ åäö','E-63-2-001 rev B'],required:['SAPPHIRE-729'],kind:'required'},
 {id:'blank-light',size:24,lines:[],kind:'blank'},
 {id:'blank-dark',size:24,dark:true,lines:[],kind:'blank'},
 {id:'low-contrast',size:16,foreground:'#bdbdbd',lines:['SAPPHIRE-729 Räksmörgås ÅÄÖ åäö'],kind:'quality'},
 {id:'wide-tile-seam',size:32,width:19000,height:300,lines:[],draw:[{x:100,y:80,text:'ALPHA-777'},{x:9870,y:80,text:'SAPPHIRE-729'},{x:15070,y:80,text:'ORBIT-00123'},{x:18000,y:170,text:'DELTA-777'}],required:['ALPHA-777','SAPPHIRE-729','ORBIT-00123','DELTA-777'],kind:'required'},
 {id:'portrait-tile-seam',size:32,width:900,height:19000,lines:[],draw:[{x:40,y:80,text:'ALPHA-777'},{x:40,y:9984,text:'SAPPHIRE-729'},{x:40,y:14984,text:'ORBIT-00123'},{x:40,y:18000,text:'DELTA-777'}],required:['ALPHA-777','SAPPHIRE-729','ORBIT-00123','DELTA-777'],kind:'required'}
);
for(const item of cases){
 const width=item.width||1400,height=item.height||350;await page.setViewportSize({width,height});
 await page.setContent('<style>html,body{margin:0;background:transparent}canvas{display:block}</style><canvas></canvas>');
 await page.evaluate(({item,width,height})=>{const canvas=document.querySelector('canvas');canvas.width=width;canvas.height=height;const ctx=canvas.getContext('2d');if(!item.transparent){ctx.fillStyle=item.dark?'#18161a':'#fff';ctx.fillRect(0,0,width,height);}ctx.fillStyle=item.foreground||(item.dark?'#f5f1f8':'#151515');ctx.font=`${item.size}px "DejaVu Sans", sans-serif`;ctx.textBaseline='top';item.lines.forEach((text,i)=>ctx.fillText(text,36,35+i*(item.size+18)));for(const row of item.draw||[])ctx.fillText(row.text,row.x,row.y);},{item,width,height});
 item.file=item.id+(item.jpeg?'.jpg':'.png');
 await page.screenshot({path:resolve(out,item.file),omitBackground:true,...(item.jpeg?{type:'jpeg',quality:85}:{type:'png'})});
 item.expected=item.lines.join('\n')+(item.draw||[]).map(row=>row.text).join('\n');item.width=width;item.height=height;
}
for(const [id,bytes] of [['empty',Buffer.alloc(0)],['corrupt',Buffer.from('not an image åäö')],['truncated-png',Buffer.from([137,80,78,71,13,10,26,10])]]){
 const file=id+'.png';await writeFile(resolve(out,file),bytes);cases.push({id,file,kind:'invalid'});
}
await writeFile(resolve(out,'manifest.json'),JSON.stringify(cases,null,2)+'\n');await browser.close();console.log(`Created ${cases.length} synthetic OCR fixtures in ${out}`);
