import { InjectionToken } from "@gitbutler/core/context";
import { shallowCompare } from "@gitbutler/shared/compare";
import { chipToasts } from "@gitbutler/ui/components/chipToast/chipToastStore";
import { createEntityAdapter, createSlice } from "@reduxjs/toolkit";
import type { PromptAttachment } from "$lib/codegen/types";
import type { ClientState } from "$lib/state/clientState.svelte";

export const ATTACHMENT_SERVICE: InjectionToken<AttachmentService> = new InjectionToken(
	"PromptAttachmentsService",
);

export class AttachmentService {
	private state = $state.raw(promptattachmentSlice.getInitialState());

	constructor(private clientState: ClientState) {
		const getSlice = clientState.injectPersistedSlice(promptattachmentSlice);

		$effect(() => {
			this.state = getSlice() ?? promptattachmentSlice.getInitialState();
		});
	}

	getByBranch(branchName?: string) {
		return (
			promptAttachmentSelectors.selectById(this.state, branchName || "default")?.attachments || []
		);
	}

	clearByBranch(branchName?: string) {
		return this.clientState.dispatch(promptattachmentSlice.actions.remove(branchName || "default"));
	}

	removeByBranch(branchName: string, attachment: PromptAttachment) {
		let attachments = promptAttachmentSelectors.selectById(this.state, branchName)?.attachments;
		if (!attachments) {
			return;
		}
		attachments = attachments.filter((f) => !shallowCompare(attachment, f));
		this.clientState.dispatch(promptattachmentSlice.actions.upsert({ branchName, attachments }));
	}

	add(branchName: string, items: PromptAttachment[]): void {
		// Check total file count
		let attachments = this.getByBranch(branchName).slice();

		if (!attachments) {
			attachments = [];
		}

		if (attachments.length + items.length > MAX_FILES) {
			chipToasts.error(`Cannot add ${items.length} files. Maximum of ${MAX_FILES} files allowed.`);
			return;
		}
		for (const item of items) {
			if (attachments.find((a) => shallowCompare(item, a))) {
				chipToasts.error(`Item is already attached.`);
				continue;
			}
			attachments.push(item);
		}

		// Add new files
		this.clientState.dispatch(promptattachmentSlice.actions.upsert({ branchName, attachments }));
	}
}

export type PromptAttachmentRecord = { branchName: string; attachments: PromptAttachment[] };

export const promptAttachmentAdapter = createEntityAdapter<PromptAttachmentRecord, string>({
	selectId: (a) => a.branchName,
});

export const promptattachmentSlice = createSlice({
	name: "promptAttachment",
	initialState: promptAttachmentAdapter.getInitialState(),
	reducers: {
		upsert: promptAttachmentAdapter.upsertOne,
		remove: promptAttachmentAdapter.removeOne,
	},
});

export const promptAttachmentSelectors = promptAttachmentAdapter.getSelectors();

/// Maximum number of allowed attachments in one prompt.
const MAX_FILES = 10;
