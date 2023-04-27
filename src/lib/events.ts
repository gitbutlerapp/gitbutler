import { createNanoEvents } from 'nanoevents';

export default () =>
	createNanoEvents<{
		goto: (path: string) => void;
		openCommandPalette: () => void;
		closeCommandPalette: () => void;
		openNewProjectModal: () => void;
		openQuickCommitModal: () => void;
	}>();
