// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces

declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}

	declare interface Navigator {
		userAgentData?: {
			getHighEntropyValues(hints: string[]): Promise<{
				readonly brands?: { readonly brand: string; readonly version: string }[];
				readonly mobile?: boolean;
				readonly platform?: string;
				readonly architecture?: string;
				readonly bitness?: string;
				readonly formFactor?: string[];
				readonly model?: string;
				readonly platformVersion?: string;
			}>;
		};
	}
}

export {};
