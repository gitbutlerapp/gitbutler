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
}

// Vite raw import type declaration
declare module "*?raw" {
	const content: string;
	export default content;
}

export {};
