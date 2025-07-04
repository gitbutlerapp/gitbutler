import { sleep } from '$lib/utils/sleep';
import type { UiState } from '$lib/state/uiState.svelte';

type Target = 'stack' | 'new-stack' | 'details' | 'diff';

export class IntelligentScrollingService {
	private targets: {
		element: Element;
		target: Target;
		projectId: string;
		stackId: string;
	}[] = [];

	constructor(private readonly uiState: UiState) {}

	register(target: { element: Element; target: Target; projectId: string; stackId: string }) {
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
		const target = this.targets.find(
			(t) => t.projectId === projectId && t.target === 'stack' && t.stackId === targetId
		);

		if (!target) return;

		this.scroll(target.element);
	}

	/** Handles clicking on either a commit or branch header */
	async show(projectId: string, stackId: string, type: Target) {
		await sleep(15);

		const target = this.targets.find(
			(t) => t.projectId === projectId && t.target === type && t.stackId === stackId
		);

		if (!target) return;

		this.scroll(target.element);
	}

	private scroll(element: Element) {
		element.scrollIntoView({ behavior: 'smooth' });
	}
}

export function scrollingAttachment(
	intelligentScrollingService: IntelligentScrollingService,
	projectId: string,
	stackId: string,
	target: Target
) {
	return (el: Element) => {
		const registration = intelligentScrollingService.register({
			element: el,
			target,
			projectId,
			stackId
		});

		return () => registration();
	};
}
