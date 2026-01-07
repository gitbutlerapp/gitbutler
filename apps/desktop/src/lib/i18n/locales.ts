import deDE from '$lib/i18n/locales/de-DE';
import enUS from '$lib/i18n/locales/en-US';
import zhCN from '$lib/i18n/locales/zh-CN';
import type { DefineLocaleMessage } from '$lib/i18n/i18nLocale';

export const supportedLocales = [
	{ code: 'en-US', name: 'English', nativeName: 'English' },
	{ code: 'de-DE', name: 'German', nativeName: 'Deutsch' },
	{ code: 'zh-CN', name: 'Simplified Chinese', nativeName: '简体中文' }
] as const;

export type SupportedLocales = (typeof supportedLocales)[number]['code'];

export const messages: {
	[K in SupportedLocales]: DefineLocaleMessage;
} = {
	'en-US': enUS,
	'de-DE': deDE,
	'zh-CN': zhCN
};
