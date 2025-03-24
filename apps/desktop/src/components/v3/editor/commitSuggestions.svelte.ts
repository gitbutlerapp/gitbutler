import { AIService } from '$lib/ai/service';
import GhostTextPlugin from '@gitbutler/ui/richText/plugins/GhostText.svelte';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { FileChange } from '$lib/ai/types';
import type { ChangeDiff } from '$lib/hunks/diffService.svelte';

export default class CommitSuggestions {
	private _ghostTextComponent = $state<ReturnType<typeof GhostTextPlugin> | undefined>();
	private textUpToAnchor = $state<string | undefined>();
	private textAfterAnchor = $state<string | undefined>();
	private lastSentMessage = $state<string | undefined>();
	private lasSelectedGhostText = $state<string | undefined>();
	private stagedChanges = $state<FileChange[] | undefined>();
	private _suggestOnType = $state<boolean>(true);
	private canUseAI = $state<boolean>(false);

	constructor(private readonly aiService: AIService) {}

	setCanUseAI(value: boolean) {
		this.canUseAI = value;
	}

	setStagedChanges(changes: ChangeDiff[]) {
		this.stagedChanges = changes
			.map((change) => {
				if (change.diff.type !== 'Patch') return;
				return {
					path: change.path,
					diffs: change.diff.subject.hunks.map((hunk) => hunk.diff)
				};
			})
			.filter(isDefined);
	}

	private async suggest(force?: boolean) {
		const text = this.textUpToAnchor ?? '';
		if (!this.canUseAI) return;
		if (!this.stagedChanges || this.stagedChanges.length === 0) return;
		if (this.lasSelectedGhostText && text.endsWith(this.lasSelectedGhostText)) return;
		if (this.lastSentMessage === text) return;
		if (!text && !force) {
			this._ghostTextComponent?.reset();
			return;
		}

		this.lastSentMessage = text;
		const autoCompletion = await this.aiService.autoCompleteCommitMessage({
			currentValue: text,
			suffix: this.textAfterAnchor ?? '',
			stagedChanges: this.stagedChanges
		});

		if (autoCompletion) {
			this._ghostTextComponent?.setText(autoCompletion);
		}
	}

	private canSuggestOnType(text: string): boolean {
		// Only suggest on type enabled and not on new line.
		return this._suggestOnType && ['\n', '\r', '.'].every((char) => !text.endsWith(char));
	}

	async onChange(textUpToAnchor: string | undefined, textAfterAnchor: string | undefined) {
		if (!textUpToAnchor) return;
		this.textUpToAnchor = textUpToAnchor;
		this.textAfterAnchor = textAfterAnchor;

		if (this.canSuggestOnType(this.textUpToAnchor)) {
			this.suggest();
		}
	}

	onKeyDown(event: KeyboardEvent | null): boolean {
		if (this._suggestOnType) return false;
		if (!event) return false;
		if (event.key === 'g' && (event.ctrlKey || event.metaKey)) {
			this.suggest(true);
			return true;
		}
		return false;
	}

	onAcceptSuggestion(text: string) {
		this.lasSelectedGhostText = text;
	}

	get suggestOnType() {
		return this._suggestOnType;
	}

	toggleSuggestOnType() {
		this._suggestOnType = !this._suggestOnType;
	}

	get ghostTextComponent(): ReturnType<typeof GhostTextPlugin> | undefined {
		return this._ghostTextComponent;
	}

	set ghostTextComponent(value: ReturnType<typeof GhostTextPlugin>) {
		this._ghostTextComponent = value;
	}
}
