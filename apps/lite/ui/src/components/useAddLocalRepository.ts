import { Toast } from "@base-ui/react";
import { useNavigate } from "@tanstack/react-router";
import { useAddProject } from "#ui/api/mutations.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { writeLastOpenedProject } from "#ui/project.ts";

type AddProjectOutcome = Awaited<ReturnType<typeof window.lite.addProject>>;
type AddProjectFailure = Exclude<AddProjectOutcome, { type: "added" | "alreadyExists" }>;

const failureMessage = (failure: AddProjectFailure): string => {
	switch (failure.type) {
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
	}
};

// Must be called from a component that outlives the button: the flow spans a
// native dialog and a mutation, and the picker-dialog footer hosting the
// button unmounts as soon as the dialog closes.
export const useAddLocalRepository = () => {
	const navigate = useNavigate();
	const toastManager = Toast.useToastManager();
	const { isPending, mutateAsync } = useAddProject();

	const addLocalRepository = async () => {
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
			description: failureMessage(outcome),
		});
	};

	return { addLocalRepository, isPending };
};
