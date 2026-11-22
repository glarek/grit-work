import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('portal updates verify signed downloads and version-bound hashes before installation', async () => {
  const source = await readFile(new URL('../src-tauri/src/commands/updater.rs', import.meta.url), 'utf8');
  assert.match(source, /login\.gritprojects\.se/);
  assert.doesNotMatch(source, /gh-proxy|api\.github\.com/);
  assert.match(source, /Policy::none/);
  assert.match(source, /INSTALL_LOCK\s*\.try_lock/);
  assert.match(source, /update.version != expected/);
  assert.match(source, /valid_download/);
  assert.ok(source.search(/update\s*\.download\(/) < source.indexOf('Sha256::digest(&bytes)'));
  assert.ok(source.indexOf('Sha256::digest(&bytes)') < source.indexOf('update.install(bytes)'));
});
