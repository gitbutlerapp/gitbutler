// Type declarations for Storybook plugins missing type definitions
declare module '@storybook/sveltekit/vite-plugin' {
	import type { Plugin } from 'vite';
	export function storybookSveltekitPlugin(): Plugin[];
}
