export interface CaptureSettings {
  interval_seconds: number;
  display_mode: 'active_window' | 'all';
  jpeg_quality: number;
  max_image_width: number;
  width_mode: 'auto' | 'fixed';
}
export interface CaptureMonitor {
  name: string;
  width: number;
  height: number;
  x: number;
  y: number;
  scale_factor: number;
}
export function normalizeCapture(raw: unknown): CaptureSettings {
  const v = (raw && typeof raw === 'object' ? raw : {}) as Partial<CaptureSettings>;
  return {
    interval_seconds: Number(v.interval_seconds) || 300,
    display_mode: v.display_mode === 'all' ? 'all' : 'active_window',
    jpeg_quality: Number(v.jpeg_quality) || 85,
    max_image_width: Math.max(0, Number(v.max_image_width) || 0),
    width_mode: v.width_mode === 'fixed' ? 'fixed' : 'auto',
  };
}
export function normalizeMonitors(raw: unknown): CaptureMonitor[] {
  if (!Array.isArray(raw)) return [];
  return raw.filter(v => v && Number.isFinite(v.width) && v.width > 0 && Number.isFinite(v.height) && v.height > 0)
    .map(v => ({ name: String(v.name || 'Skärm'), width: v.width, height: v.height,
      x: Number(v.x) || 0, y: Number(v.y) || 0, scale_factor: Number(v.scale_factor) || 1 }));
}
/** Planning range, not a disk quota: JPEG content and compression vary substantially. */
export function estimateCapture(settings: CaptureSettings, monitors: CaptureMonitor[], hours = 8) {
  const screens = monitors.length ? monitors : [{ name: 'Exempel', width: 1920, height: 1080, x: 0, y: 0, scale_factor: 1 }];
  const maxWidth = settings.width_mode === 'fixed' ? settings.max_image_width
    : Math.min(Math.max(screens[0].width * 0.7, 1280), Math.max(settings.max_image_width, 1280) * 2);
  let width: number, height: number;
  if (settings.display_mode === 'all') {
    width = Math.max(...screens.map(m => m.x + m.width)) - Math.min(...screens.map(m => m.x));
    height = Math.max(...screens.map(m => m.y + m.height)) - Math.min(...screens.map(m => m.y));
  } else {
    // The active monitor can change; budget for the largest detected screen.
    const screen = [...screens].sort((a, b) => b.width * b.height - a.width * a.height)[0];
    width = screen.width; height = screen.height;
  }
  const ratio = settings.max_image_width === 0 ? 1 : Math.min(1, Math.max(800, maxWidth) / width);
  const pixels = width * height * ratio * ratio;
  const quality = (Math.min(95, Math.max(30, settings.jpeg_quality)) / 85) ** 2;
  const count = Math.ceil(Math.min(24, Math.max(1, hours)) * 3600 / Math.max(30, settings.interval_seconds));
  return { count, low: count * (pixels * 0.08 * quality + 4096), high: count * (pixels * 0.45 * quality + 4096), assumedScreen: !monitors.length };
}
export function formatStorage(bytes: number): string {
  const unit = bytes >= 1e9 ? 'GB' : 'MB';
  return `${(bytes / (unit === 'GB' ? 1e9 : 1e6)).toLocaleString('sv-SE', { maximumFractionDigits: 1 })} ${unit}`;
}
