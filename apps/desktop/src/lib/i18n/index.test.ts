import { locale, localeForSystemLanguages, setLocale, t } from "$lib/i18n";
import en from "$lib/i18n/locales/en.json";
import zhCN from "$lib/i18n/locales/zh-CN.json";
import zhTW from "$lib/i18n/locales/zh-TW.json";
import { afterEach, expect, test } from "vitest";

afterEach(() => setLocale("en"));

test("translates English and Simplified Chinese messages", () => {
	expect(t("create-branch")).toBe("Create branch");
	setLocale("zh-CN");
	expect(t("create-branch")).toBe("创建分支");
});

test("notifies subscribers when the language changes", () => {
	const values: string[] = [];
	const unsubscribe = locale.subscribe(() => values.push(t("create-branch")));
	setLocale("zh-CN");
	unsubscribe();
	expect(values).toEqual(["Create branch", "创建分支"]);
});

test("preserves unknown keys", () => {
	expect(t("missing-key")).toBe("missing-key");
});

test("falls back to English for missing translations", () => {
	const original = zhCN["create-branch"];
	try {
		Reflect.deleteProperty(zhCN, "create-branch");
		setLocale("zh-CN");
		expect(t("create-branch")).toBe(en["create-branch"]);
	} finally {
		zhCN["create-branch"] = original;
	}
});

test("interpolates string and numeric parameters and preserves missing parameters", () => {
	const key = "are-you-sure-you-want-to-delete-the-local-changes-inside-the";
	expect(t(key, { branchName: "feature" })).toBe(
		"Are you sure you want to delete the local changes inside the branch feature?",
	);
	expect(t(key, { branchName: 42 })).toContain("42");
	expect(t(key, {})).toContain("{branchName}");
});

test("Chinese dictionary covers the English keys and placeholders", () => {
	expect(Object.keys(zhCN).sort()).toEqual(Object.keys(en).sort());
	for (const key of Object.keys(en) as (keyof typeof en)[]) {
		expect(zhCN[key].match(/\{\w+\}/g)).toEqual(en[key].match(/\{\w+\}/g));
	}
});

test("translates Traditional Chinese with matching keys and placeholders", () => {
	setLocale("zh-TW");
	expect(t("create-branch")).toBe("建立分支");
	expect(Object.keys(zhTW).sort()).toEqual(Object.keys(en).sort());
	for (const key of Object.keys(en) as (keyof typeof en)[]) {
		expect(zhTW[key].match(/\{\w+\}/g)).toEqual(en[key].match(/\{\w+\}/g));
	}
});

test.each([
	[["zh-CN"], "zh-CN"],
	[["zh-SG"], "zh-CN"],
	[["zh-TW"], "zh-TW"],
	[["yue-HK"], "zh-TW"],
	[["zh-MO"], "zh-TW"],
	[["zh-Hant-CN"], "zh-TW"],
	[["zh-Hans-TW"], "zh-CN"],
	[["ZH-hAnT-HK"], "zh-TW"],
	[["fr-FR", "zh-TW"], "zh-TW"],
	[["en-US", "zh-CN"], "en"],
	[["fr-FR"], "en"],
	[[], "en"],
])("selects a supported locale for %j", (languages, expected) => {
	expect(localeForSystemLanguages(languages)).toBe(expected);
});
