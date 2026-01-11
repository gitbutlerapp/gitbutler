import deDE from '$lib/i18n/locales/de-DE';
import enUS from '$lib/i18n/locales/en-US';
import jaJP from '$lib/i18n/locales/ja-JP';
import zhCN from '$lib/i18n/locales/zh-CN';
import type { DefineLocaleMessage } from '$lib/i18n/i18nLocale';

export const supportedLocales = [
	{ code: 'en-US', name: 'English', nativeName: 'English' },
	{ code: 'de-DE', name: 'German', nativeName: 'Deutsch' },
	{ code: 'zh-CN', name: 'Simplified Chinese', nativeName: '简体中文' },
	{ code: 'ja-JP', name: 'Japanese', nativeName: '日本語' }
] as const;

export type SupportedLocales = (typeof supportedLocales)[number]['code'];

export const messages: {
	[K in SupportedLocales]: DefineLocaleMessage;
} = {
	'en-US': enUS,
	'de-DE': deDE,
	'zh-CN': zhCN,
	'ja-JP': jaJP
};
