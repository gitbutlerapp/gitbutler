import type { InteractiveIntegrationStep } from "$lib/stacks/stack";

export function getStepIndex(steps: InteractiveIntegrationStep[], stepId: string): number {
	return steps.findIndex((step) => step.subject.id === stepId);
}

export function canShiftStepUp(steps: InteractiveIntegrationStep[], stepId: string): boolean {
	return getStepIndex(steps, stepId) > 0;
}

export function canShiftStepDown(steps: InteractiveIntegrationStep[], stepId: string): boolean {
	const index = getStepIndex(steps, stepId);
	return index !== -1 && index < steps.length - 1;
}

export function swapSteps(
	steps: InteractiveIntegrationStep[],
	indexA: number,
	indexB: number,
): InteractiveIntegrationStep[] {
	const newSteps = structuredClone(steps);
	const stepA = newSteps[indexA];
	const stepB = newSteps[indexB];
	if (!stepA || !stepB) return steps;
	newSteps[indexA] = stepB;
	newSteps[indexB] = stepA;
	return newSteps;
}

export function shiftStepUp(
	steps: InteractiveIntegrationStep[],
	stepId: string,
): InteractiveIntegrationStep[] {
	const currentIndex = getStepIndex(steps, stepId);
	if (currentIndex > 0) {
		return swapSteps(steps, currentIndex, currentIndex - 1);
	}
	return steps;
}

export function shiftStepDown(
	steps: InteractiveIntegrationStep[],
	stepId: string,
): InteractiveIntegrationStep[] {
	const currentIndex = getStepIndex(steps, stepId);
	if (currentIndex !== -1 && currentIndex < steps.length - 1) {
		return swapSteps(steps, currentIndex, currentIndex + 1);
	}
	return steps;
}

export function updateStepType(
	steps: InteractiveIntegrationStep[],
	stepId: string,
	commitId: string,
	newType: "pick" | "skip",
): InteractiveIntegrationStep[] {
	return steps.map((step) => {
		if (step.subject.id === stepId) {
			return { type: newType, subject: { id: stepId, commitId } };
		}
		return step;
	});
}

export function pickUpstreamStep(
	steps: InteractiveIntegrationStep[],
	stepId: string,
	commitId: string,
	upstreamCommitId: string,
): InteractiveIntegrationStep[] {
	return steps.map((step) => {
		if (step.subject.id === stepId) {
			return {
				type: "pickUpstream",
				subject: { id: stepId, commitId, upstreamCommitId },
			};
		}
		return step;
	});
}

export function pickLocalStep(
	steps: InteractiveIntegrationStep[],
	stepId: string,
	commitId: string,
): InteractiveIntegrationStep[] {
	return steps.map((step) => {
		if (step.subject.id === stepId) {
			return { type: "pick", subject: { id: stepId, commitId } };
		}
		return step;
	});
}

export function getStepCommitInfo(step: InteractiveIntegrationStep): {
	id: string;
	commitIds: string[];
} {
	const id = step.subject.id;
	switch (step.type) {
		case "pickUpstream":
			return { id, commitIds: [step.subject.upstreamCommitId] };
		case "pick":
		case "skip":
			return { id, commitIds: [step.subject.commitId] };
		case "squash":
			return { id, commitIds: step.subject.commits };
	}
}

/**
 * Squash a step into the one below it. The caller must pre-fetch the squash message.
 */
export function squashStepInto(
	steps: InteractiveIntegrationStep[],
	stepId: string,
	commitIds: string[],
	squashMessage: string,
): InteractiveIntegrationStep[] {
	const stepIndex = steps.findIndex((step) => step.subject.id === stepId);
	const isValidSquashOperation = stepIndex !== -1 && stepIndex < steps.length - 1;
	if (!isValidSquashOperation) return steps;

	const newSteps = structuredClone(steps);
	const stepToBeSquashedInto = newSteps[stepIndex + 1];
	if (!stepToBeSquashedInto) return steps;

	const targetStepInfo = getStepCommitInfo(stepToBeSquashedInto);
	const combinedCommits = [...commitIds, ...targetStepInfo.commitIds];

	newSteps.splice(stepIndex, 2, {
		type: "squash",
		subject: {
			id: targetStepInfo.id,
			commits: combinedCommits,
			message: squashMessage,
		},
	});
	return newSteps;
}

/**
 * Split a squash step at a commit boundary. The caller must pre-fetch group messages
 * for groups with more than one commit.
 */
export function splitStepAtCommit(
	steps: InteractiveIntegrationStep[],
	stepId: string,
	commitId: string,
	firstGroupMessage: string,
	secondGroupMessage: string,
): InteractiveIntegrationStep[] {
	const stepIndex = steps.findIndex((step) => step.subject.id === stepId);
	if (stepIndex === -1) return steps;

	const newSteps = structuredClone(steps);
	const stepToSplit = newSteps[stepIndex];
	if (!stepToSplit || stepToSplit.type !== "squash") return steps;

	const { commits } = stepToSplit.subject;
	const commitIndex = commits.indexOf(commitId);
	if (commits.length <= 1 || !commits.includes(commitId) || commitIndex === -1) return steps;

	const firstGroup = commits.slice(0, commitIndex);
	const secondGroup = commits.slice(commitIndex);
	if (firstGroup.length === 0) return steps;

	if (firstGroup.length === 1) {
		newSteps[stepIndex] = {
			type: "pick",
			subject: { id: stepId, commitId: firstGroup[0]! },
		};
	} else {
		newSteps[stepIndex] = {
			type: "squash",
			subject: {
				id: stepId,
				commits: firstGroup,
				message: firstGroupMessage,
			},
		};
	}

	if (secondGroup.length === 1) {
		newSteps.splice(stepIndex + 1, 0, {
			type: "pick" as const,
			subject: {
				id: crypto.randomUUID(),
				commitId: secondGroup[0]!,
			},
		});
	} else {
		newSteps.splice(stepIndex + 1, 0, {
			type: "squash" as const,
			subject: {
				id: crypto.randomUUID(),
				commits: secondGroup,
				message: secondGroupMessage,
			},
		});
	}

	return newSteps;
}
