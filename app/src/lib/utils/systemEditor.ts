import { invoke } from '@tauri-apps/api/tauri';

class SystemEditor {
	constructor() {
		this.resolveEditorVariant();
	}

	static instance = new SystemEditor();

	private systemEditor = '';

	async resolveEditorVariant() {
		this.systemEditor = (await invoke('resolve_vscode_variant')) as string;
	}

	get() {
		return this.systemEditor;
	}
}

const editor = SystemEditor.instance;

export { editor };
