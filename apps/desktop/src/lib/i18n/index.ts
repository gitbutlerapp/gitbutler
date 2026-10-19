/**
 * Lightweight i18n module for the desktop app.
 *
 * Messages live in `locales/*.json`; English is the source of truth.
 * Callers can subscribe to `locale` to update translated text when it changes.
 */

import en from "$lib/i18n/locales/en.json";
import zhCN from "$lib/i18n/locales/zh-CN.json";
import zhTW from "$lib/i18n/locales/zh-TW.json";
import { get, writable } from "svelte/store";

export type AppLocale = "en" | "zh-CN" | "zh-TW";

export const APP_LOCALES: { value: AppLocale; label: string }[] = [
	{ value: "en", label: "English" },
	{ value: "zh-CN", label: "简体中文" },
	{ value: "zh-TW", label: "繁體中文" },
];

const dictionaries: Record<AppLocale, Record<string, string>> = {
	en: en as Record<string, string>,
	"zh-CN": zhCN as Record<string, string>,
	"zh-TW": zhTW as Record<string, string>,
};

/** Current locale store. Defaults to English until the caller selects a language. */
export const locale = writable<AppLocale>("en");

/**
 * Translate a message key with optional `{placeholder}` interpolation.
 * Falls back to the English entry, then to the key itself, so a missing
 * translation never renders as an empty string.
 */
export function t(key: string, params?: Record<string, string | number>): string {
	const active = get(locale);
	const message = dictionaries[active]?.[key] ?? dictionaries.en[key] ?? key;
	if (!params) return message;
	return message.replace(/\{(\w+)\}/g, (match, name) =>
		name in params ? String(params[name]) : match,
	);
}

/** Change the translation language. Persistence is managed by callers. */
export function setLocale(next: AppLocale) {
	locale.set(next);
}

/** Select the first supported system language, falling back to English. */
export function localeForSystemLanguages(
	languages: readonly string[] = navigator.languages?.length
		? navigator.languages
		: [navigator.language],
): AppLocale {
	for (const tag of languages) {
		const [language, ...subtags] = tag.toLowerCase().split("-");
		if (language === "en") return "en";
		if (language !== "zh" && language !== "yue") continue;
		// An explicit script takes precedence over the region.
		if (subtags.includes("hant")) return "zh-TW";
		if (subtags.includes("hans")) return "zh-CN";
		return subtags.some((part) => ["tw", "hk", "mo"].includes(part)) ? "zh-TW" : "zh-CN";
	}
	return "en";
}
