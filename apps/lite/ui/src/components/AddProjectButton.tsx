import { Toast } from "@base-ui/react";
import { useNavigate } from "@tanstack/react-router";
import type { FC } from "react";
import { useAddProject } from "#ui/api/mutations.ts";
import { getButtonClassName, type ButtonSize } from "#ui/components/Button.tsx";
import { errorMessageForToast } from "#ui/errors.ts";
import { writeLastOpenedProject } from "#ui/project.ts";

type AddProjectOutcome = Awaited<ReturnType<typeof window.lite.addProject>>;

const outcomeError = (outcome: AddProjectOutcome): string => {
	switch (outcome.type) {
		case "pathNotFound":
			return "The selected path no longer exists.";
		case "notADirectory":
			return "The selected path is not a directory.";
		case "bareRepository":
			return "Bare repositories are not supported.";
		case "nonMainWorktree":
			return "Only a repository's main worktree can be added.";
		case "noWorkdir":
			return "The selected repository has no working directory.";
		case "noDotGitDirectory":
			return "The selected directory has no .git directory.";
		case "reftableRefFormatUnsupported":
			return "Repositories using reftable references are not supported.";
		case "notAGitRepository":
			return "The selected directory is not a Git repository.";
		case "added":
		case "alreadyExists":
			return "";
	}
};

type Props = {
	testId: string;
	size?: ButtonSize;
	onBeforePick?: () => void;
};

export const AddProjectButton: FC<Props> = ({ testId, size, onBeforePick }) => {
	const navigate = useNavigate();
	const toastManager = Toast.useToastManager();
	const { isPending, mutateAsync } = useAddProject();

	const addProject = async () => {
		onBeforePick?.();

		let path: string | null;
		try {
			path = await window.lite.pickDirectory();
		} catch (error) {
			toastManager.add({
				type: "error",
				title: "Failed to open repository picker",
				description: errorMessageForToast(error),
			});
			return;
		}

		if (path === null) return;

		let outcome: AddProjectOutcome;
		try {
			outcome = await mutateAsync(path);
		} catch {
			return;
		}

		if (outcome.type === "added" || outcome.type === "alreadyExists") {
			writeLastOpenedProject(outcome.subject.id);
			void navigate({
				to: "/project/$id/workspace",
				params: { id: outcome.subject.id },
			});
			return;
		}

		toastManager.add({
			type: "error",
			title: "Could not add project",
			description: outcomeError(outcome),
		});
	};

	return (
		<button
			type="button"
			className={getButtonClassName({ size })}
			data-testid={testId}
			disabled={isPending}
			onClick={() => void addProject()}
		>
			{isPending ? "Adding repository…" : "Add local repository"}
		</button>
	);
};
