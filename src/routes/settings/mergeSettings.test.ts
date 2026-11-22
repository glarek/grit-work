import assert from 'node:assert/strict';
import test from 'node:test';
import { mergeSettingsEdits } from './mergeSettings.ts';

test('saving appearance and idle edits preserves newer capture and privacy settings', () => {
  const baseline = { screenshot_interval:300, storage:{jpeg_quality:85}, idle_threshold_minutes:5,
    background_opacity:1, theme:'light', privacy:{app_rules:[],excluded_keywords:[]} };
  const draft = {...baseline, idle_threshold_minutes:9, background_opacity:0.6};
  const latest = {...baseline, screenshot_interval:60, storage:{jpeg_quality:90}, theme:'dark',
    privacy:{app_rules:[],excluded_keywords:['private']} };
  const result = mergeSettingsEdits(latest,draft,baseline);
  assert.equal(result.screenshot_interval,60);
  assert.equal(result.storage.jpeg_quality,90);
  assert.equal(result.theme,'dark');
  assert.equal(result.idle_threshold_minutes,9);
  assert.equal(result.background_opacity,0.6);
  assert.deepEqual(result.privacy.excluded_keywords,['private']);
  assert.equal(latest.background_opacity,1);
});

test('privacy edits across tabs keep unrelated concurrent rule changes', () => {
  const baseline = {daily_work_goal_minutes:480,goal_notifications:false,privacy:{app_rules:['old'],excluded_keywords:['a']}};
  const draft = {...baseline,daily_work_goal_minutes:420,goal_notifications:true,privacy:{...baseline.privacy,app_rules:['new']}};
  const latest = {...baseline,privacy:{...baseline.privacy,excluded_keywords:['b']}};
  const result = mergeSettingsEdits(latest,draft,baseline);
  assert.deepEqual(result.privacy,{app_rules:['new'],excluded_keywords:['b']});
  assert.equal(result.daily_work_goal_minutes,420);
  assert.equal(result.goal_notifications,true);
});
