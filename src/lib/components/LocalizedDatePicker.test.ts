import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
test('Shared calendar uses Swedish, Monday first and ISO date values for single and range modes',async()=>{
  const s=await readFile(new URL('./LocalizedDatePicker.svelte',import.meta.url),'utf8');
  assert.match(s,/Calendar, RangeCalendar, Popover/);
  assert.equal((s.match(/weekStartsOn=\{0\}/g)||[]).length,2);
  assert.equal((s.match(/locale="sv-SE"/g)||[]).length,2);
  assert.match(s,/parseDate\(v\)/);
  assert.match(s,/value=v\.toString\(\)/);
  assert.doesNotMatch(s,/type="date"/);
  assert.match(s,/minValue=\{parsed\(min\)\}/);
  assert.match(s,/maxValue=\{parsed\(max\)\}/);
});
