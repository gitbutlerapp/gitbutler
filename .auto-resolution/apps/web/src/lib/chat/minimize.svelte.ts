const SHORTCUT_KEY = 'b';

export class ChatMinimize {
	private _minimize = $state<boolean>(false);

	toggle() {
		this._minimize = !this._minimize;
	}

	isKeyboardShortcut(event: KeyboardEvent) {
		return event.key === SHORTCUT_KEY && (event.ctrlKey || event.metaKey);
	}

	expand() {
		this._minimize = false;
	}

	get value() {
		return this._minimize;
	}
}
