declare global {
	interface CloseWatcher extends EventTarget {
		destroy(): void;
		requestClose(): void;
	}

	var CloseWatcher: {
		new (): CloseWatcher;
		prototype: CloseWatcher;
	};
}

export {};
