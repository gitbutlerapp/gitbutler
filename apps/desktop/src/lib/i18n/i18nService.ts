import { messages, type SupportedLocales } from '$lib/i18n/locales';
import { InjectionToken } from '@gitbutler/core/context';
import {
	compile,
	type CoreContext,
	type CoreOptions,
	createCoreContext,
	datetime,
	fallbackWithLocaleChain,
	number,
	registerLocaleFallbacker,
	registerMessageCompiler,
	registerMessageResolver,
	resolveValue,
	translate
} from '@intlify/core';
import { derived, get, type Readable, writable, type Writable } from 'svelte/store';
import type { DefineLocaleMessage } from '$lib/i18n/i18nLocale';

// Register intlify core components
registerMessageCompiler(compile);
registerMessageResolver(resolveValue);
registerLocaleFallbacker(fallbackWithLocaleChain);

export type TranslateFunction = (key: string, params?: any) => string;
export type DateTimeFormatFunction = (value: number | Date, format?: string | any) => string;
export type NumberFormatFunction = (value: number, format?: string | any) => string;

export type I18nOptions = CoreOptions<
	string,
	{
		message: DefineLocaleMessage;
	},
	SupportedLocales
>;

export const I18N_SERVICE = new InjectionToken<I18nService>('I18nService');

export class I18nService {
	private readonly context: CoreContext;
	private readonly localeStore: Writable<string>;

	readonly t: Readable<TranslateFunction>;
	readonly d: Readable<DateTimeFormatFunction>;
	readonly n: Readable<NumberFormatFunction>;

	readonly defaultLocale = 'en-US';

	constructor(savedLocale?: string) {
		const initialLocale = savedLocale || this.defaultLocale;
		const options: I18nOptions = {
			locale: initialLocale,
			fallbackLocale: this.defaultLocale,
			messages,
			warnHtmlMessage: false,
			escapeParameter: true
		};

		this.context = createCoreContext<DefineLocaleMessage, SupportedLocales, string, I18nOptions>(
			options
		);

		this.localeStore = writable(initialLocale);
		this.localeStore.subscribe((value) => {
			this.context.locale = value;
		});

		this.t = derived(this.localeStore, () => (key: string, params?: any): string => {
			return translate(this.context, key, params) as string;
		});

		this.d = derived(
			this.localeStore,
			() =>
				(value: number | Date, format?: string | any): string => {
					return datetime(this.context, value, format) as string;
				}
		);

		this.n = derived(this.localeStore, () => (value: number, format?: string | any): string => {
			return number(this.context, value, format) as string;
		});
	}

	get locale(): Writable<string> {
		return this.localeStore;
	}

	getLocale(): string {
		return get(this.localeStore);
	}

	setLocale(locale: string): void {
		this.localeStore.set(locale);
	}
}
