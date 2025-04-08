import 'vite/types/customEvent.d.ts';

declare module 'vite/types/customEvent.d.ts' {
	interface CustomEventMap {
		'gb:reload': void;
	}
}
