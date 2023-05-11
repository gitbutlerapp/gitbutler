import { createNanoEvents } from 'nanoevents';

type Events = {
	goto: (path: string) => void;
	openCommandPalette: () => void;
	closeCommandPalette: () => void;
	openNewProjectModal: () => void;
	openQuickCommitModal: () => void;
};

const events = createNanoEvents<Events>();

export const on = <K extends keyof Events>(event: K, callback: Events[K]) => {
	console.debug('subscribe', event);
	const unsubscribe = events.on(event, callback);
	return () => {
		console.debug('unsubscribe', event);
		unsubscribe();
	};
};

export const emit = <K extends keyof Events>(event: K, ...args: Parameters<Events[K]>) => {
	console.debug('event', event, args);
	events.emit(event, ...args);
};
