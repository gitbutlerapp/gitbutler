import { createNanoEvents } from 'nanoevents';

interface Events {
	goto: (path: string) => void;
	openCommandPalette: () => void;
	closeCommandPalette: () => void;
	openNewProjectModal: () => void;
	openQuickCommitModal: () => void;
}

export default () => {
	const emitter = createNanoEvents<Events>();
	return {
		on: (...args: Parameters<(typeof emitter)['on']>) => emitter.on(...args),

		goto: (path: string) => emitter.emit('goto', path),
		openCommandPalette: () => emitter.emit('openCommandPalette'),
		openNewProjectModal: () => emitter.emit('openNewProjectModal'),
		openQuickCommitModal: () => emitter.emit('openQuickCommitModal'),
		closeCommandPalette: () => emitter.emit('closeCommandPalette')
	};
};
