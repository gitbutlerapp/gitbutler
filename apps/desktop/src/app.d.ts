// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces

declare module 'tinykeys';

declare namespace App {
	interface Error {
		message: string;
		errorId?: string;
		errorCode?: string;
	}
}
