import test from 'node:test';
import assert from 'node:assert/strict';
import {reportingChoicesValid,type ReportingContext} from './reportingContext';
import {normalizeGritReport} from './gritTypes';
const report=normalizeGritReport({date:'2026-11-18',status:'draft',rows:[{id:'r',project_id:2,activity_id:3,time_code_id:4,article_id:5}]})!;
const context:ReportingContext={date:report.date,user:{id:1,name:'Test'},checked_at:1,projects:[{project_id:2,active:true,member:true,activities:[{id:3,name:'Arbete'}],time_codes:[{id:4,name:'Normal'}],articles:[{id:5,name:'Tid'}],restriction_source:'active_registries_no_project_restrictions_exposed'}],existing_rows:[],previous_rows:[]};
test('Reporting permits explicit choices from the current project registry',()=>assert.equal(reportingChoicesValid(report,context),true));
test('Each metadata choice is required and must be active for this project',()=>{
 for(const key of ['activity_id','time_code_id','article_id'])for(const value of [0,99])assert.equal(reportingChoicesValid({...report,rows:[{...report.rows[0]!,[key]:value}]},context),false);
});
test('Unverified, mismatched, inactive and nonmember project contexts cannot submit',()=>{
 assert.equal(reportingChoicesValid(report,null),false);
 assert.equal(reportingChoicesValid(report,{...context,date:'2026-11-17'}),false);
 for(const key of ['member','active'])assert.equal(reportingChoicesValid(report,{...context,projects:[{...context.projects[0]!,[key]:false}]}),false);
 assert.equal(reportingChoicesValid(report,{...context,projects:[]}),false);
});
test('Same-day entries block posting; historical examples never become report evidence',()=>{
 const row={id:9,date:report.date,project:null,activity:null,time_code:null,article:null,hours:1,comment:'Test'};
 assert.equal(reportingChoicesValid(report,{...context,existing_rows:[row]}),false);
 assert.equal(reportingChoicesValid(report,{...context,previous_rows:[row]}),true);
 assert.equal(report.rows.length,1);
});
test('Legacy row normalization defaults time code to zero and preserves an explicit code',()=>{
 assert.equal(normalizeGritReport({date:report.date,rows:[{}]})!.rows[0]?.time_code_id,0);
 assert.equal(normalizeGritReport(report)!.rows[0]?.time_code_id,4);
});
