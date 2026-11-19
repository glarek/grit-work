import test from 'node:test';
import assert from 'node:assert/strict';
import { get } from 'svelte/store';
import type {
  TranslationDictionary,
} from './index.ts';

type I18nModule = typeof import('./index.ts');
type EnvironmentName = 'window' | 'navigator' | 'document';
type TestEnvironment = Partial<Record<EnvironmentName, unknown>>;
type LeafSignature = [path: string, type: string, placeholders: string[]];

interface TestStorage extends Storage {
  writes: Array<[key: string, value: string]>;
}

let importSequence = 0;

async function importFresh(label: string): Promise<I18nModule> {
  importSequence += 1;
  const moduleUrl = new URL(
    `./index.ts?${label}=${Date.now()}-${importSequence}`,
    import.meta.url,
  );
  return import(moduleUrl.href);
}

async function withEnvironment<T>(
  environment: TestEnvironment,
  callback: () => Promise<T>,
): Promise<T> {
  const names: readonly EnvironmentName[] = ['window', 'navigator', 'document'];
  const descriptors = new Map(
    names.map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]),
  );

  try {
    for (const name of names) {
      if (Object.prototype.hasOwnProperty.call(environment, name)) {
        Object.defineProperty(globalThis, name, {
          configurable: true,
          writable: true,
          value: environment[name],
        });
      } else {
        Reflect.deleteProperty(globalThis, name);
      }
    }
    return await callback();
  } finally {
    for (const name of names) {
      const descriptor = descriptors.get(name);
      if (descriptor) {
        Object.defineProperty(globalThis, name, descriptor);
      } else {
        Reflect.deleteProperty(globalThis, name);
      }
    }
  }
}

function createStorage(initialValue: string | null = null): TestStorage {
  let value = initialValue;
  const writes: Array<[key: string, value: string]> = [];
  return {
    writes,
    get length() {
      return value === null ? 0 : 1;
    },
    clear: () => {
      value = null;
    },
    getItem: () => value,
    key: (index) => (index === 0 && value !== null ? 'work-review.locale' : null),
    removeItem: () => {
      value = null;
    },
    setItem: (key: string, nextValue: string) => {
      writes.push([key, nextValue]);
      value = nextValue;
    },
  };
}

test('初始化语言应按显式值、持久化值、浏览器语言和默认值排序', async () => {
  const cases = [
    {
      preferred: 'en-US',
      stored: 'ar',
      browserLanguages: ['zh-HK'],
      expected: 'sv-SE',
    },
    {
      preferred: null,
      stored: 'ar-EG',
      browserLanguages: ['zh-HK'],
      expected: 'sv-SE',
    },
    {
      preferred: '',
      stored: 'ar',
      browserLanguages: ['en-US'],
      expected: 'sv-SE',
    },
    {
      preferred: null,
      stored: null,
      browserLanguages: ['zh-HK'],
      expected: 'sv-SE',
    },
    {
      preferred: null,
      stored: null,
      browserLanguages: ['fr-FR', 'en-US'],
      expected: 'sv-SE',
    },
  ];

  for (const [index, testCase] of cases.entries()) {
    const storage = createStorage(testCase.stored);
    await withEnvironment({
      window: { localStorage: storage },
      navigator: {
        languages: testCase.browserLanguages,
        language: testCase.browserLanguages[0],
      },
    }, async () => {
      const i18n = await importFresh(`initialize-${index}`);
      assert.equal(i18n.initializeLocale(testCase.preferred), testCase.expected);
      assert.equal(get(i18n.locale), testCase.expected);
      assert.deepEqual(storage.writes.at(-1), [
        'work-review.locale',
        testCase.expected,
      ]);
    });
  }

  await withEnvironment({}, async () => {
    const i18n = await importFresh('initialize-default');
    assert.equal(i18n.initializeLocale(), 'sv-SE');
  });
});

test('语言设置、循环和文档方向应使用规范化后的 locale', async () => {
  const storage = createStorage();
  const documentElement = { lang: '', dir: '' };

  await withEnvironment({
    window: { localStorage: storage },
    document: { documentElement },
  }, async () => {
    const i18n = await importFresh('set-cycle-document');

    assert.equal(i18n.setLocale('zh-HK'), 'sv-SE');
    assert.equal(i18n.cycleLocale(), 'sv-SE');
    assert.equal(i18n.cycleLocale(), 'sv-SE');
    assert.equal(i18n.setLocale('ar-EG'), 'sv-SE');
    i18n.applyLocaleToDocument();
    assert.deepEqual(documentElement, { lang: 'sv-SE', dir: 'ltr' });

    i18n.applyLocaleToDocument('en-US');
    assert.deepEqual(documentElement, { lang: 'sv-SE', dir: 'ltr' });
    assert.equal(i18n.getLocaleShortLabel('zh-HK'), 'SV');
    assert.equal(i18n.getLocaleLabel('en-US'), 'Svenska');
  });
});

test('SSR 与 localStorage 读写异常不应中断语言初始化', async () => {
  await withEnvironment({}, async () => {
    const i18n = await importFresh('ssr');
    assert.equal(i18n.initializeLocale(), 'sv-SE');
    assert.doesNotThrow(() => i18n.applyLocaleToDocument('ar'));
  });

  const failingStorage: Storage = {
    length: 0,
    clear: () => {},
    getItem: () => {
      throw new Error('read failed');
    },
    key: () => null,
    removeItem: () => {},
    setItem: () => {
      throw new Error('write failed');
    },
  };

  await withEnvironment({
    window: {
      localStorage: failingStorage,
    },
    navigator: { languages: ['en-US'], language: 'en-US' },
  }, async () => {
    const i18n = await importFresh('storage-errors');
    assert.equal(i18n.initializeLocale(), 'sv-SE');
    assert.equal(i18n.setLocale('ar'), 'sv-SE');
  });
});

test('翻译函数应支持回退、插值、数组消息和缺失键', async () => {
  await withEnvironment({}, async () => {
    const i18n = await importFresh('translations');
    const { default: swedish } = await import('./locales/sv-SE.ts');
    const originalExpandAll = swedish.common.expandAll;

    try {
      i18n.setLocale('en');
      assert.equal(i18n.t('common.expandAll', { count: 3 }), 'Visa alla 3');
      assert.equal(
        i18n.t('common.expandAll'),
        'Visa alla {count}',
        '未提供参数时应保留占位符',
      );
      assert.equal(i18n.t('missing.deep.key'), 'missing.deep.key');
      assert.equal(i18n.t('ask.starterPrompts'), 'ask.starterPrompts');
      assert.ok(Array.isArray(i18n.tm('ask.starterPrompts')));
      assert.equal(
        i18n.tm('ask.starterPrompts.0'),
        swedish.ask.starterPrompts[0],
      );
      assert.equal(
        i18n.t('ask.starterPrompts.0'),
        swedish.ask.starterPrompts[0],
      );
      assert.equal(typeof i18n.tm('report.blockNames.CATEGORY_TABLE'), 'string');
      assert.equal(i18n.tm('missing.deep.key'), undefined);

      swedish.common.expandAll = '{first}';
      assert.equal(
        i18n.t('common.expandAll', { first: '{second}', second: 'done' }),
        'done',
      );
      swedish.common.expandAll = originalExpandAll;

      Reflect.deleteProperty(swedish.common, 'expandAll');
      assert.equal(i18n.t('common.expandAll', { count: 2 }), 'common.expandAll');
    } finally {
      swedish.common.expandAll = originalExpandAll;
    }
  });
});

test('日期、时间和时长格式应跟随当前 locale', async () => {
  await withEnvironment({}, async () => {
    const i18n = await importFresh('formatting');
    const date = new Date('2024-01-02T03:04:05Z');
    const dateOptions: Intl.DateTimeFormatOptions = {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      timeZone: 'UTC',
    };
    const timeOptions: Intl.DateTimeFormatOptions = {
      hour: '2-digit',
      minute: '2-digit',
      timeZone: 'UTC',
    };

    i18n.setLocale('en');
    assert.equal(i18n.formatLocalizedDate(date, dateOptions), '2024-01-02');
    assert.equal(i18n.formatIsoDate(date, 'UTC'), '2024-01-02');
    assert.equal(i18n.swedishWeekdayName(date, 'long', 'UTC'), 'tisdag');
    const sunday = new Date(2024, 0, 7);
    assert.equal(i18n.startOfIsoWeek(sunday).getDay(), 1);
    assert.equal(i18n.formatIsoDate(i18n.startOfIsoWeek(sunday)), '2024-01-01');
    assert.equal(
      i18n.formatLocalizedTime(date, timeOptions),
      new Intl.DateTimeFormat('sv-SE', timeOptions).format(date),
    );
    const swedishDurations = ['0 s', '0 min', '45 sekunder', '2 minuter', '1 timme', '1 tim 1 min'];
    for (const requested of ['sv-SE', 'en', 'zh-CN', 'zh-TW', 'ar'] as const) {
      i18n.setLocale(requested);
      assert.equal(get(i18n.locale), 'sv-SE');
      assert.deepEqual([
        i18n.formatDurationLocalized(0),
        i18n.formatDurationLocalized(0, { compact: true }),
        i18n.formatDurationLocalized(45),
        i18n.formatDurationLocalized(120),
        i18n.formatDurationLocalized(3_600),
        i18n.formatDurationLocalized(3_660, { compact: true }),
      ], swedishDurations, requested);
    }
  });
});

test('分类翻译应命中当前语言并对未知值原样回退', async () => {
  await withEnvironment({}, async () => {
    const i18n = await importFresh('categories');
    i18n.setLocale('en');

    assert.equal(i18n.translateCategoryLabel('development'), 'Utveckling');
    assert.equal(i18n.translateCategoryLabel('custom'), 'custom');
    assert.equal(i18n.translateSemanticCategoryLabel('编码开发'), 'Utveckling');
    assert.equal(i18n.translateSemanticCategoryLabel('自定义活动'), '自定义活动');
  });
});

function collectLeafSignatures(
  value: TranslationDictionary,
  prefix = '',
  output: LeafSignature[] = [],
): LeafSignature[] {
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (child && typeof child === 'object' && !Array.isArray(child)) {
      collectLeafSignatures(child, path, output);
      continue;
    }

    const strings: string[] = Array.isArray(child) ? child : [child];
    const placeholders = [...new Set(
      strings.flatMap((item) => (
        typeof item === 'string'
          ? [...item.matchAll(/\{([^{}]+)\}/g)]
              .map((match) => match[1])
              .filter((placeholder): placeholder is string => placeholder !== undefined)
          : []
      )),
    )].sort();
    output.push([
      path,
      Array.isArray(child) ? 'string[]' : typeof child,
      placeholders,
    ]);
  }
  return output;
}

test('四语言词典的叶路径、类型与占位符应完全一致', async () => {
  const localeNames = ['zh-CN', 'en', 'zh-TW', 'ar', 'sv-SE'] as const;
  const dictionaries: TranslationDictionary[] = await Promise.all(
    localeNames.map(async (localeName) => (
      (await import(`./locales/${localeName}.ts`)).default
    )),
  );
  const byPath = (left: LeafSignature, right: LeafSignature) => (
    left[0].localeCompare(right[0])
  );
  const firstDictionary = dictionaries[0];
  assert.ok(firstDictionary);
  const expected = collectLeafSignatures(firstDictionary).sort(byPath);

  assert.equal(expected.length, 1_339);
  for (let index = 1; index < dictionaries.length; index += 1) {
    const dictionary = dictionaries[index];
    assert.ok(dictionary);
    assert.deepEqual(
      collectLeafSignatures(dictionary).sort(byPath),
      expected,
      `${localeNames[index]} 的词典结构或占位符与 zh-CN 不一致`,
    );
  }
});
