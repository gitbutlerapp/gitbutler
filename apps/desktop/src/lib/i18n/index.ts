/**
 * Lightweight i18n module for the desktop app.
 *
 * Messages live in `locales/*.json`; `en.json` is the source of truth and
 * every other locale falls back to it, then to the key itself, so a missing
 * translation never renders as an empty string.
 *
 * The `t()` helper is reactive on the current locale, so Svelte components
 * re-render when the language changes at runtime. To add a language, drop a
 * new `locales/<tag>.json` next to the others and register it in
 * `APP_LOCALES` below.
 */

import type { Writable } from "svelte/store";
import en from "$lib/i18n/locales/en.json";
import zhCN from "$lib/i18n/locales/zh-CN.json";
import zhTW from "$lib/i18n/locales/zh-TW.json";
import { setUiLocale } from "@gitbutler/ui-svelte/utils/timeAgo";
import { get, writable } from "svelte/store";

export type AppLocale = "en" | "zh-CN" | "zh-TW";

/** All languages offered in Settings > General, in display order. */
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

/** Current locale store. Persisted by the uiState slice; `setLocale` syncs it. */
export const locale: Writable<AppLocale> = writable<AppLocale>("en");

/**
 * Translate a message key with optional `{placeholder}` interpolation.
 * Falls back to the English entry, then to the key itself.
 */
export function t(key: string, params?: Record<string, string | number>): string {
	const active = get(locale);
	const message = dictionaries[active]?.[key] ?? dictionaries.en[key] ?? key;
	if (!params) return message;
	return message.replace(/\{(\w+)\}/g, (match, name) =>
		name in params ? String(params[name]) : match,
	);
}

/** Change the app language. Callers persist the choice via uiState. */
export function setLocale(next: AppLocale) {
	locale.set(next);
	setUiLocale(next);
}

/**
 * Pick the best supported locale for the system's languages: the first
 * navigator tag whose primary language subtag matches a supported locale.
 * Chinese script subtags (Hant/HK/TW/MO) map to Traditional, others to
 * Simplified.
 */
export function localeForSystemLanguages(): AppLocale {
	for (const tag of navigator.languages ?? [navigator.language]) {
		if (!tag) continue;
		const [language = "", region = "", script = ""] = tag.split("-");
		const isChinese = ["zh", "yue"].includes(language.toLowerCase());
		if (isChinese) {
			const variant = (script || region || "").toLowerCase();
			const traditional = ["hant", "tw", "hk", "mo"].includes(variant);
			return traditional ? "zh-TW" : "zh-CN";
		}
		const match = APP_LOCALES.find(({ value }) => value.toLowerCase() === language.toLowerCase());
		if (match) return match.value;
	}
	return "en";
}
