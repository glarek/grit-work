import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
test('Sidebar has one Swedish navigation, accessible recording action and today image count',async()=>{
 const s=await readFile(new URL('./Sidebar.svelte',import.meta.url),'utf8');
 assert.match(s,/GRIT Work/);assert.match(s,/lucide-svelte/);
 assert.match(s,/aria-current=\{active \? 'page' : undefined\}/);
 assert.match(s,/aria-label="Huvudnavigation"/);
 assert.match(s,/get_today_stats/);assert.match(s,/screenshot_count/);assert.match(s,/bilder idag/);
 assert.match(s,/Samlar in/);assert.match(s,/Pausad/);assert.match(s,/aria-label=\{isPaused/);
 assert.doesNotMatch(s,/locale-switch|theme-switch|path:\s*'\/node'|sidebar\.tagline/);
});
