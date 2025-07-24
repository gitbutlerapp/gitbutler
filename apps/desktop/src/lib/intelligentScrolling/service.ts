import { sleep } from '$lib/utils/sleep';
import { InjectionToken } from '@gitbutler/shared/context';
import type { UiState } from '$lib/state/uiState.svelte';

export type TargetType = 'stack' | 'new-stack' | 'details' | 'diff';

export const INTELLIGENT_SCROLLING_SERVICE = new InjectionToken<IntelligentScrollingService>(
	'IntelligentScrollingService'
);

export class IntelligentScrollingService {
	private targets: {
		element: Element;
		type: TargetType;
		id: string;
	}[] = [];

	constructor(private readonly uiState: UiState) {}

	register(target: { element: Element; type: TargetType; id: string }) {
		this.targets.push(target);
		return () => {
			this.targets = this.targets.filter((t) => t !== target);
		};
	}

	async unassignedFileClicked(projectId: string) {
		await sleep(15);

		const projectState = this.uiState.project(projectId);

		const committingState = projectState.exclusiveAction?.current;
		if (committingState?.type !== 'commit') return;

		const targetId = committingState.stackId;
		const target = this.targets.find((t) => t.type === 'stack' && t.id === targetId);

		if (!target) return;

		this.scroll(target.element);
	}

	/** Handles clicking on either a commit or branch header */
	async show(projectId: string, stackId: string, type: TargetType) {
		await sleep(15);

		const target = this.targets.find((t) => t.type === type && t.id === stackId);

		if (!target) return;

		this.scroll(target.element);
	}

	private scroll(element: Element) {
		element.scrollIntoView({ behavior: 'smooth' });
	}
}

export function scrollingAttachment(
	intelligentScrollingService: IntelligentScrollingService,
	id?: string,
	type?: TargetType
) {
	if (!id || !type) return;
	return (el: Element) => {
		const registration = intelligentScrollingService.register({
			element: el,
			type,
			id
		});

		return () => registration();
	};
}
