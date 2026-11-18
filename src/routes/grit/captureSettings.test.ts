import test from 'node:test';
import assert from 'node:assert/strict';
import { estimateCapture, normalizeCapture, normalizeMonitors } from './captureSettings.ts';
const monitors = [
  { name: 'Left', width: 1920, height: 1080, x: -1920, y: 0, scale_factor: 1 },
  { name: 'Right', width: 1920, height: 1080, x: 0, y: 0, scale_factor: 1 },
];
test('storage budget follows interval, active hours, and number of screens', () => {
  const settings = { ...normalizeCapture(null), width_mode: 'fixed' as const, max_image_width: 7680 };
  const one = estimateCapture(settings, monitors, 8);
  const two = estimateCapture({ ...settings, display_mode: 'all' }, monitors, 8);
  assert.equal(one.count, 96);
  assert.equal(two.count, 96); // one combined image each time
  assert.ok(two.low > one.low && two.high > one.high);
  assert.equal(estimateCapture({ ...settings, interval_seconds: 150 }, monitors, 8).high, one.high * 2);
  assert.equal(estimateCapture(settings, monitors, 24).high, one.high * 3);
});
test('rotated vertical displays and downscaling affect estimated pixels', () => {
  const settings = { ...normalizeCapture(null), width_mode: 'fixed' as const, display_mode: 'all' as const, max_image_width: 3840 };
  const vertical = [{ name: 'Portrait', width: 1080, height: 1920, x: 0, y: -1920, scale_factor: 1 }];
  assert.ok(estimateCapture(settings, vertical).high > 0);
  assert.ok(estimateCapture({ ...settings, max_image_width: 800 }, monitors).high < estimateCapture(settings, monitors).high);
  assert.equal(estimateCapture(settings, []).assumedScreen, true);
  assert.deepEqual(normalizeMonitors([{ width: -1, height: 10 }, null]), []);
});
