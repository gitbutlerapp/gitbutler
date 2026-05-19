import type {
	InitialBranchIntegration,
	InteractiveIntegration,
	InteractiveIntegrationStep,
} from "@gitbutler/but-sdk";

export type CommitPickerOption = {
	id: string;
	subject: string;
	refs: Array<string>;
	group: "Local" | "Upstream" | "Shared";
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
			commitIds: [string, string];
			message: string;
	  };

const createStepId = (): string => crypto.randomUUID();

const firstCommitId = (commitOptions: Array<CommitPickerOption>): string => {
	const [first] = commitOptions;
	if (!first) throw new Error("No commits are available for integration.");
	return first.id;
};

const secondCommitId = (
	commitOptions: Array<CommitPickerOption>,
	selectedCommitId: string,
): string => {
	const second = commitOptions.find((option) => option.id !== selectedCommitId);
	return second?.id ?? selectedCommitId;
};

export const buildCommitPickerOptions = (
	initialIntegration: InitialBranchIntegration,
): Array<CommitPickerOption> => [
	...initialIntegration.divergence.localOnly.map((commit) => ({
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
	...initialIntegration.divergence.matched.map((commit) => ({
		id: commit.id,
		subject: commit.subject,
		refs: commit.refs,
		group: "Shared" as const,
	})),
];

export const buildIntegrationStepDrafts = (
	integration: InteractiveIntegration,
): Array<IntegrationStepDraft> =>
	integration.steps.map((step) => {
		switch (step.kind) {
			case "pick":
			case "merge":
				return {
					id: createStepId(),
					kind: step.kind,
					commitId: step.commit_id,
				};
			case "squash":
				return {
					id: createStepId(),
					kind: "squash",
					commitIds: [step.commits[0] ?? "", step.commits[1] ?? step.commits[0] ?? ""],
					message: step.message ?? "",
				};
		}
	});

export const createDefaultIntegrationStepDraft = (
	commitOptions: Array<CommitPickerOption>,
): IntegrationStepDraft => ({
	id: createStepId(),
	kind: "pick",
	commitId: firstCommitId(commitOptions),
});

export const changeIntegrationStepDraftKind = ({
	step,
	kind,
	commitOptions,
}: {
	step: IntegrationStepDraft;
	kind: InteractiveIntegrationStep["kind"];
	commitOptions: Array<CommitPickerOption>;
}): IntegrationStepDraft => {
	if (kind === step.kind) return step;

	if (kind === "squash") {
		const selectedCommitId = step.kind === "squash" ? step.commitIds[0] : step.commitId;
		const primaryCommitId =
			selectedCommitId !== "" ? selectedCommitId : firstCommitId(commitOptions);
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
		commitId: commitId !== "" ? commitId : firstCommitId(commitOptions),
	};
};

export const updateIntegrationStepDraftCommit = ({
	step,
	commitId,
	index,
	commitOptions,
}: {
	step: IntegrationStepDraft;
	commitId: string;
	index?: 0 | 1;
	commitOptions: Array<CommitPickerOption>;
}): IntegrationStepDraft => {
	if (step.kind !== "squash") return { ...step, commitId };

	const nextCommitIds: [string, string] = [...step.commitIds];
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
};

export const updateIntegrationStepDraftMessage = ({
	step,
	message,
}: {
	step: IntegrationStepDraft;
	message: string;
}): IntegrationStepDraft => {
	if (step.kind !== "squash") return step;
	return { ...step, message };
};

export const reorderIntegrationStepDrafts = ({
	steps,
	draggedStepId,
	destinationIndex,
}: {
	steps: Array<IntegrationStepDraft>;
	draggedStepId: string;
	destinationIndex: number;
}): Array<IntegrationStepDraft> => {
	const sourceIndex = steps.findIndex((step) => step.id === draggedStepId);
	if (sourceIndex === -1) return steps;

	const nextSteps = [...steps];
	const [draggedStep] = nextSteps.splice(sourceIndex, 1);
	if (!draggedStep) return steps;

	const insertionIndex = sourceIndex < destinationIndex ? destinationIndex - 1 : destinationIndex;
	nextSteps.splice(insertionIndex, 0, draggedStep);
	return nextSteps;
};

export const buildInteractiveIntegration = ({
	mergeBase,
	steps,
}: {
	mergeBase: string;
	steps: Array<IntegrationStepDraft>;
}): InteractiveIntegration => ({
	mergeBase,
	steps: steps.map((step) => {
		switch (step.kind) {
			case "pick":
			case "merge":
				return {
					kind: step.kind,
					commit_id: step.commitId,
				};
			case "squash":
				return {
					kind: "squash",
					commits: step.commitIds,
					message: step.message === "" ? null : step.message,
				};
		}
	}),
});
