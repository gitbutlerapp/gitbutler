import type {
	InitialBranchIntegration,
	IntegrationDivergenceCommit,
	InteractiveIntegration,
	InteractiveIntegrationStep,
} from "@gitbutler/but-sdk";

export type CommitPickerOption = {
	id: string;
	subject: string;
	refs: string[];
	group: "Local" | "Upstream";
};

export type IntegrationStepDraft =
	| {
			id: string;
			kind: "pick" | "merge";
			commitId: string;
	  }
	| {
			id: string;
			kind: "squash";
			commitIds: string[];
			message: string;
	  };

function createStepId() {
	return crypto.randomUUID();
}

function firstCommitId(commitOptions: CommitPickerOption[]): string {
	const first = commitOptions[0];
	if (!first) throw new Error("No commits are available for integration.");
	return first.id;
}

function secondCommitId(commitOptions: CommitPickerOption[], selectedCommitId: string): string {
	const second = commitOptions.find((option) => option.id !== selectedCommitId);
	return second?.id ?? selectedCommitId;
}

function isIntegrated(commit: Pick<IntegrationDivergenceCommit, "targetRelation">): boolean {
	return commit.targetRelation.kind !== "notIntegrated";
}

export function buildCommitPickerOptions(
	initialIntegration: InitialBranchIntegration,
): CommitPickerOption[] {
	return [
		...initialIntegration.divergence.localOnly
			.filter((commit) => !isIntegrated(commit))
			.map((commit) => ({
				id: commit.id,
				subject: commit.subject,
				refs: commit.refs,
				group: "Local" as const,
			})),
		...initialIntegration.divergence.upstreamOnly.map((commit) => ({
			id: commit.id,
			subject: commit.subject,
			refs: commit.refs,
			group: "Upstream" as const,
		})),
	];
}

export function buildIntegrationStepDrafts(
	integration: InteractiveIntegration,
): IntegrationStepDraft[] {
	return integration.steps.map((step) => {
		switch (step.kind) {
			case "pick":
			case "merge":
				return {
					id: createStepId(),
					kind: step.kind,
					commitId: step.commitId,
				};
			case "squash":
				return {
					id: createStepId(),
					kind: "squash",
					commitIds: [...step.commits],
					message: step.message ?? "",
				};
		}
	});
}

export function createDefaultIntegrationStepDraft(
	commitOptions: CommitPickerOption[],
): IntegrationStepDraft {
	return {
		id: createStepId(),
		kind: "pick",
		commitId: firstCommitId(commitOptions),
	};
}

export function changeIntegrationStepDraftKind({
	step,
	kind,
	commitOptions,
}: {
	step: IntegrationStepDraft;
	kind: InteractiveIntegrationStep["kind"];
	commitOptions: CommitPickerOption[];
}): IntegrationStepDraft {
	if (kind === step.kind) return step;

	if (kind === "squash") {
		const selectedCommitId = step.kind === "squash" ? step.commitIds[0] : step.commitId;
		const primaryCommitId =
			selectedCommitId !== undefined && selectedCommitId !== ""
				? selectedCommitId
				: firstCommitId(commitOptions);
		return {
			id: step.id,
			kind: "squash",
			commitIds: [primaryCommitId, secondCommitId(commitOptions, primaryCommitId)],
			message: "",
		};
	}

	const commitId = step.kind === "squash" ? step.commitIds[0] : step.commitId;
	return {
		id: step.id,
		kind,
		commitId: commitId !== undefined && commitId !== "" ? commitId : firstCommitId(commitOptions),
	};
}

export function updateIntegrationStepDraftCommit({
	step,
	commitId,
	index,
	commitOptions,
}: {
	step: IntegrationStepDraft;
	commitId: string;
	index?: number;
	commitOptions: CommitPickerOption[];
}): IntegrationStepDraft {
	if (step.kind !== "squash") return { ...step, commitId };

	const nextCommitIds = [...step.commitIds];
	const targetIndex = index ?? 0;
	nextCommitIds[targetIndex] = commitId;
	if (targetIndex === 0 && nextCommitIds[1] === commitId)
		nextCommitIds[1] = secondCommitId(commitOptions, commitId);
	if (targetIndex === 1 && nextCommitIds[0] === commitId)
		nextCommitIds[0] = secondCommitId(commitOptions, commitId);

	return {
		...step,
		commitIds: nextCommitIds,
	};
}

export function updateIntegrationStepDraftMessage({
	step,
	message,
}: {
	step: IntegrationStepDraft;
	message: string;
}): IntegrationStepDraft {
	if (step.kind !== "squash") return step;
	return { ...step, message };
}

export function reorderIntegrationStepDrafts({
	steps,
	draggedStepId,
	destinationIndex,
}: {
	steps: IntegrationStepDraft[];
	draggedStepId: string;
	destinationIndex: number;
}): IntegrationStepDraft[] {
	const sourceIndex = steps.findIndex((step) => step.id === draggedStepId);
	if (sourceIndex === -1) return steps;

	const nextSteps = [...steps];
	const [draggedStep] = nextSteps.splice(sourceIndex, 1);
	if (!draggedStep) return steps;

	const insertionIndex = sourceIndex < destinationIndex ? destinationIndex - 1 : destinationIndex;
	nextSteps.splice(insertionIndex, 0, draggedStep);
	return nextSteps;
}

export function buildInteractiveIntegration({
	mergeBase,
	firstLocalNotIntegrated,
	steps,
}: {
	mergeBase: string;
	firstLocalNotIntegrated: string | null;
	steps: IntegrationStepDraft[];
}): InteractiveIntegration {
	return {
		mergeBase,
		firstLocalNotIntegrated,
		steps: steps.map((step) => {
			switch (step.kind) {
				case "pick":
				case "merge":
					return {
						kind: step.kind,
						commitId: step.commitId,
					};
				case "squash":
					return {
						kind: "squash",
						commits: step.commitIds,
						message: step.message === "" ? null : step.message,
					};
			}
		}),
	};
}
