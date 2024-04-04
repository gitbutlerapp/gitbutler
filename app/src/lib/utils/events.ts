import { createNanoEvents } from 'nanoevents';

type Events = {
	goto: (path: string) => void;
	openSendIssueModal: () => void;
	openBookmarkModal: () => void;
	createBookmark: (params: { projectId: string }) => void;
};

const events = createNanoEvents<Events>();

export function on<K extends keyof Events>(event: K, callback: Events[K]) {
	const unsubscribe = events.on(event, callback);
	return () => {
		unsubscribe();
	};
}

export function emit<K extends keyof Events>(event: K, ...args: Parameters<Events[K]>) {
	events.emit(event, ...args);
}
