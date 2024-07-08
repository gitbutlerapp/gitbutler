// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		interface Error {
			message: string;
			errorId?: string;
		}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}
}

declare module 'tinykeys';

interface HTMLElement {
	scrollIntoViewIfNeeded: (centerIfNeeded?: boolean) => void;
}

export {};
