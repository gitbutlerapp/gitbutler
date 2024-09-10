import { persisted } from '$lib/persisted/persisted';
import { get, type Readable } from 'svelte/store';
import type { Project } from '$lib/backend/projects';
import type { GitHostIssueService } from '$lib/gitHost/interface/gitHostIssueService';

export type Topic = {
	title: string;
	body: string;
	hasIssue: boolean;
	createdAt: number;
	id: string;
};

export class TopicService {
	topics = persisted<Topic[]>([], this.localStorageKey);

	constructor(
		private project: Project,
		private issueService: Readable<GitHostIssueService | undefined>
	) {}

	private get localStorageKey(): string {
		return `TopicService--${this.project.id}`;
	}

	create(title: string, body: string, hasIssue: boolean = false): Topic {
		const topic = {
			title,
			body,
			hasIssue,
			createdAt: Date.now(),
			id: crypto.randomUUID()
		};

		this.topics.set([topic, ...get(this.topics)]);

		return topic;
	}

	update(topic: Topic) {
		const filteredTopics = get(this.topics).filter((storedTopic) => storedTopic.id !== topic.id);

		this.topics.set([topic, ...filteredTopics]);
	}

	remove(topic: Topic) {
		const filteredTopics = get(this.topics).filter((storedTopic) => storedTopic.id !== topic.id);

		this.topics.set(filteredTopics);
	}
}
