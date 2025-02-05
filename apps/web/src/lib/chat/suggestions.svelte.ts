import type { User } from '$lib/user/userService';
import type { UserSimple } from '@gitbutler/shared/users/types';
import type { UserService } from '@gitbutler/shared/users/userService';
import type { MentionNodeAttrs } from '@gitbutler/ui/RichTextEditor.svelte';

export default class SuggestionsHandler {
	private _isLoading = $state<boolean>(false);

	constructor(
		private userService: UserService,
		private chatParticipants: UserSimple[] | undefined,
		private currentUser: User | undefined
	) {}

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

	get isLoading() {
		return this._isLoading;
	}
}
