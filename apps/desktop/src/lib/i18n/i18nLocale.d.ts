import type { LocaleMessage, LocaleMessageDictionary } from '@intlify/core';

export interface DefineLocaleMessage extends LocaleMessage {
	settings: LocaleMessageDictionary<{
		language: string;
	}>;
	notifications: LocaleMessageDictionary<{
		languageChanged: string;
	}>;
}
