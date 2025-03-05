import type MentionSuggestions from '$lib/components/chat/MentionSuggestions.svelte';
import type { User } from '$lib/user/userService';
import type { UserSimple } from '@gitbutler/shared/users/types';
import type { UserService } from '@gitbutler/shared/users/userService';
import type { MentionNodeAttrs, SuggestionProps } from '@gitbutler/ui/old_RichTextEditor.svelte';

export default class SuggestionsHandler {
	private _isLoading = $state<boolean>(false);
	private _suggestions = $state<MentionNodeAttrs[]>();
	private _mentionSuggestions = $state<ReturnType<typeof MentionSuggestions>>();
	private _selectSuggestion = $state<(id: MentionNodeAttrs) => void>();

	constructor(
		private userService: UserService,
		private chatParticipants: UserSimple[] | undefined,
		private currentUser: User | undefined
	) {}

	reset() {
		this._suggestions = undefined;
		this._selectSuggestion = undefined;
	}

	private async searchUsers(query: string): Promise<UserSimple[]> {
		this._isLoading = true;
		const results = await this.userService.searchUsers({
			query: {
				filters: [
					{
						field: 'login',
						operator: 'NOT_NULL'
					}
				],
				search_terms: [
					{
						value: query,
						operator: 'STARTS_WITH'
					}
				]
			}
		});

		this._isLoading = false;
		return results;
	}

	private async getSuggestionItemsForQuery(query: string): Promise<MentionNodeAttrs[]> {
		const results = await this.searchUsers(query);

		const users = results
			.map((item) => {
				if (!item.login) return undefined;
				if (item.login === this.currentUser?.login) return undefined;
				return { id: item.id.toString(), label: item.login };
			})
			.filter((item): item is MentionNodeAttrs => !!item);

		return users;
	}

	private async getInitialSuggestionItems(): Promise<MentionNodeAttrs[]> {
		const participants: UserSimple[] = this.chatParticipants ?? [];
		return participants
			.map((participant) => {
				if (!participant.login) return undefined;
				if (participant.login === this.currentUser?.login) return undefined;
				return { id: participant.id.toString(), label: participant.login };
			})
			.filter((item): item is MentionNodeAttrs => !!item);
	}

	async getSuggestionItems(query: string): Promise<MentionNodeAttrs[]> {
		if (query) {
			return await this.getSuggestionItemsForQuery(query);
		}
		return await this.getInitialSuggestionItems();
	}

	onSuggestionStart(props: SuggestionProps) {
		this._suggestions = props.items;
		this._selectSuggestion = (item: MentionNodeAttrs) => {
			props.command(item);
		};
	}

	onSuggestionUpdate(props: SuggestionProps) {
		this._suggestions = props.items;
		this._selectSuggestion = (item: MentionNodeAttrs) => {
			props.command(item);
		};
	}

	onSuggestionExit() {
		this._suggestions = undefined;
		this._selectSuggestion = undefined;
	}

	onSuggestionKeyDown(event: KeyboardEvent): boolean {
		if (event.key === 'Escape') {
			this._suggestions = undefined;
			this._selectSuggestion = undefined;
			return true;
		}

		if (event.key === 'Enter') {
			if (this._mentionSuggestions) {
				this._mentionSuggestions.onEnter();
			}
			event.preventDefault();
			event.stopPropagation();
			return true;
		}

		if (event.key === 'ArrowUp') {
			if (this._mentionSuggestions) {
				this._mentionSuggestions.onArrowUp();
			}
			return true;
		}

		if (event.key === 'ArrowDown') {
			if (this._mentionSuggestions) {
				this._mentionSuggestions.onArrowDown();
			}
			return true;
		}

		return false;
	}

	get isLoading() {
		return this._isLoading;
	}

	get suggestions() {
		return this._suggestions;
	}

	set suggestions(value: MentionNodeAttrs[] | undefined) {
		this._suggestions = value;
	}

	get selectSuggestion() {
		return this._selectSuggestion;
	}

	set selectSuggestion(value: ((id: MentionNodeAttrs) => void) | undefined) {
		this._selectSuggestion = value;
	}

	get mentionSuggestions() {
		return this._mentionSuggestions;
	}

	set mentionSuggestions(value: ReturnType<typeof MentionSuggestions> | undefined) {
		this._mentionSuggestions = value;
	}
}
