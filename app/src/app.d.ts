// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
// and what to do when importing types
declare namespace App {}

declare module 'tinykeys';

interface HTMLElement {
	scrollIntoViewIfNeeded: (centerIfNeeded?: boolean) => void;
}
