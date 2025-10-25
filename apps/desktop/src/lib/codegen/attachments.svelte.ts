import { shallowCompare } from '@gitbutler/shared/compare';
import { chipToasts } from '@gitbutler/ui';
import type { PromptAttachment } from '$lib/codegen/types';

const MAX_FILES = 10;

export class PromptAttachments {
	attachments = $state<PromptAttachment[]>([]);

	setAttachments(files: PromptAttachment[]) {
		this.attachments = files;
	}

	remove(attachment: PromptAttachment) {
		this.attachments = this.attachments.filter(
			(f) => f.type !== attachment.type || !shallowCompare(f.subject, attachment.subject)
		);
	}

	add(items: PromptAttachment[]): void {
		// Check total file count
		if (this.attachments.length + items.length > MAX_FILES) {
			chipToasts.error(`Cannot add ${items.length} files. Maximum of ${MAX_FILES} files allowed.`);
			return;
		}

		// Validate and process each file
		const newFiles: PromptAttachment[] = [];
		for (const item of items) {
			// Check for duplicates
			const isDuplicate = this.attachments.find((a) => {
				switch (a.type) {
					case 'commit':
						return a.type === item.type && a.subject.commitId === item.subject.commitId;
					case 'file':
						return a.type === item.type && a.subject.path === item.subject.path;
					case 'hunk':
						return (
							a.type === item.type &&
							a.subject.path === item.subject.path &&
							a.subject.start === item.subject.start
						);
				}
			});

			if (isDuplicate) {
				chipToasts.error(`Item is already attached.`);
				return;
			}

			newFiles.push(item);
		}

		// Add new files
		this.setAttachments([...this.attachments, ...newFiles]);
	}
}
