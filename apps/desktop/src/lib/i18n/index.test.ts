import { APP_LOCALES, locale, localeForSystemLanguages, setLocale, t } from "$lib/i18n";
import enJson from "$lib/i18n/locales/en.json";
import fs from "node:fs";
import path from "node:path";
import { get } from "svelte/store";
import { afterEach, describe, expect, test, vi } from "vitest";

// Keep dayjs side effects out of these tests; setLocale's sync is asserted
// through the mock instead.
const setUiLocale = vi.fn();
vi.mock("@gitbutler/ui-svelte/utils/timeAgo", () => ({
	setUiLocale: (tag: string) => setUiLocale(tag),
}));

// keyed loosely so the guard test can index with arbitrary scanned keys
const en = enJson as Record<string, string>;

/** All shipped dictionaries, keyed by locale tag (e.g. "zh-CN"). */
const dictionaries = readAllDictionaries();

function readAllDictionaries(): Record<string, Record<string, string>> {
	const dir = findLocaleDir();
	const out: Record<string, Record<string, string>> = {};
	for (const file of fs.readdirSync(dir)) {
		if (file.endsWith(".json"))
			out[file.replace(/\.json$/, "")] = JSON.parse(fs.readFileSync(path.join(dir, file), "utf8"));
	}
	return out;
}

function findLocaleDir(): string {
	let dir = process.cwd();
	for (let i = 0; i < 8; i++) {
		const candidate = path.join(dir, "src/lib/i18n/locales/en.json");
		if (fs.existsSync(candidate)) return path.dirname(candidate);
		const parent = path.dirname(dir);
		if (parent === dir) break;
		dir = parent;
	}
	throw new Error(`could not locate apps/desktop from ${process.cwd()}`);
}

// Every test starts from a known locale because the store is module-global.
afterEach(() => {
	setLocale("en");
	setUiLocale.mockClear();
});

describe("t", () => {
	test("returns the English message by default", () => {
		expect(t("create-branch")).toBe("Create branch");
	});

	test("returns the key itself for unknown keys so nothing renders empty", () => {
		expect(t("no-such-key")).toBe("no-such-key");
	});

	test("interpolates provided placeholders and keeps unknown ones intact", () => {
		const message = t("are-you-sure-you-want-to-delete-the-local-changes-inside-the", {
			branchName: "feature-x",
		});
		expect(message).toBe(
			"Are you sure you want to delete the local changes inside the branch feature-x?",
		);
		expect(t("are-you-sure-you-want-to-delete-the-local-changes-inside-the")).toContain(
			"{branchName}",
		);
	});

	test("stringifies numeric params", () => {
		const message = t("are-you-sure-you-want-to-delete-the-local-changes-inside-the", {
			branchName: 42,
		});
		expect(message).toContain("42");
	});
});

describe("setLocale", () => {
	test("switches the language used by t", () => {
		setLocale("zh-CN");
		expect(t("create-branch")).toBe("创建分支");
	});

	test("updates the locale store and syncs the relative-time locale", () => {
		setLocale("zh-TW");
		expect(get(locale)).toBe("zh-TW");
		expect(setUiLocale).toHaveBeenCalledWith("zh-TW");
	});

	test("every advertised locale can translate a sample key", () => {
		for (const { value } of APP_LOCALES) {
			setLocale(value);
			expect(t("create-branch")).toBeTruthy();
		}
	});
});

describe("localeForSystemLanguages", () => {
	const originalLanguages = navigator.languages;
	afterEach(() => {
		Object.defineProperty(navigator, "languages", { value: originalLanguages, configurable: true });
	});

	function withLanguages(...languages: string[]) {
		Object.defineProperty(navigator, "languages", { value: languages, configurable: true });
	}

	test("maps Simplified Chinese system languages to zh-CN", () => {
		withLanguages("zh-CN", "en-US");
		expect(localeForSystemLanguages()).toBe("zh-CN");
	});

	test("maps Traditional Chinese system languages to zh-TW", () => {
		withLanguages("zh-TW", "en-US");
		expect(localeForSystemLanguages()).toBe("zh-TW");
	});

	test("maps Cantonese (Hong Kong) to zh-TW", () => {
		withLanguages("yue-HK");
		expect(localeForSystemLanguages()).toBe("zh-TW");
	});

	test("falls back to the first matching non-Chinese language", () => {
		withLanguages("fr-FR", "en-US");
		expect(localeForSystemLanguages()).toBe("en");
	});
});

describe("dictionaries", () => {
	test("every advertised locale has a dictionary file", () => {
		for (const { value } of APP_LOCALES) {
			expect(dictionaries[value], `missing locales/${value}.json`).toBeTruthy();
		}
	});

	test("every non-English dictionary covers exactly the English key set", () => {
		const enKeys = Object.keys(en).sort();
		for (const [tag, dict] of Object.entries(dictionaries)) {
			if (tag === "en") continue;
			expect(Object.keys(dict).sort(), `key sets differ between en and ${tag}`).toEqual(enKeys);
		}
	});

	test("every t() key used in source exists in the dictionaries", () => {
		const srcRoot = path.resolve(findLocaleDir(), "../../../");
		const used = new Set<string>();
		visitFiles(srcRoot, (file) => {
			const source = fs.readFileSync(file, "utf8");
			for (const match of source.matchAll(/\bt\(\s*"([a-z0-9][a-z0-9-]*)"/g)) {
				used.add(match[1]!);
			}
		});
		// sanity: the scan must find something, otherwise it is broken
		expect(used.size).toBeGreaterThan(500);
		for (const key of used) {
			expect(en[key], `missing dictionary entry for "${key}"`).toBeTruthy();
		}
	});
});

function visitFiles(dir: string, visit: (file: string) => void) {
	for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
		if (entry.name === "node_modules" || entry.name.startsWith(".")) continue;
		const file = path.join(dir, entry.name);
		if (entry.isDirectory()) visitFiles(file, visit);
		else if (/\.(svelte|ts)$/.test(entry.name) && !entry.name.endsWith(".test.ts")) visit(file);
	}
}
