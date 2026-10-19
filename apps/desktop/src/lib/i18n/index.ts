/**
 * Lightweight i18n module for the desktop app.
 *
 * Messages live in `locales/en.json` (source of truth) and `locales/zh-CN.json`.
 * Callers can subscribe to `locale` to update translated text when it changes.
 */

import en from "$lib/i18n/locales/en.json";
import zhCN from "$lib/i18n/locales/zh-CN.json";
import { get, writable } from "svelte/store";

export type AppLocale = "en" | "zh-CN";

export const APP_LOCALES: { value: AppLocale; label: string }[] = [
	{ value: "en", label: "English" },
	{ value: "zh-CN", label: "简体中文" },
];

const dictionaries: Record<AppLocale, Record<string, string>> = {
	en: en as Record<string, string>,
	"zh-CN": zhCN as Record<string, string>,
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
