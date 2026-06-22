import { goto } from "$app/navigation";
import { showToast, showWarning } from "$lib/notifications/toasts";
import { projectPath } from "$lib/routes/routes.svelte";
import { TestId } from "@gitbutler/ui";
// Inlined to avoid circular import with forge/.
type ForgeName = "github" | "gitlab" | "bitbucket" | "azure" | "default";
import type { ApiProject, ForgeUser } from "@gitbutler/but-sdk";

export type Project = {
	id: string;
	title: string;
	description?: string;
	path: string;
	git_dir?: string;
	api?: ApiProject;
	ok_with_force_push: boolean;
	force_push_protection: boolean;
	husky_hooks_enabled: boolean;
	omit_certificate_check: boolean | undefined;
	use_diff_context: boolean | undefined;
	// Produced just for the frontend to determine if the project is open in any window.
	is_open: boolean;
	forge_override: ForgeName | undefined;
	preferred_forge_user: ForgeUser | null;
	// Gerrit mode enabled for this project, derived from git configuration
	gerrit_mode: boolean;
	/**
	 * The path to the forge review template, if set in git configuration.
	 */
	forge_review_template_path: string | null;
};

export function vscodePath(path: string) {
	return path.includes("\\") ? "/" + path.replace("\\", "/") : path;
}

export type AddProjectOutcome =
	| {
			type: "added";
			subject: Project;
	  }
	| {
			type: "alreadyExists";
			subject: Project;
	  }
	| {
			type: "pathNotFound";
	  }
	| {
			type: "notADirectory";
	  }
	| {
			type: "bareRepository";
	  }
	| {
			type: "nonMainWorktree";
	  }
	| {
			type: "noWorkdir";
	  }
	| {
			type: "noDotGitDirectory";
	  }
	| {
			type: "reftableRefFormatUnsupported";
	  }
	| {
			type: "notAGitRepository";
			/**
			 * The error message received
			 */
			subject: string;
	  };

/**
 * Correctly handle the outcome of an addProject operation by passing the project to the callback or
 * showing toasts as necessary.
 */
export function handleAddProjectOutcome(
	outcome: AddProjectOutcome,
	onAdded?: (project: Project) => void,
): true {
	switch (outcome.type) {
		case "added":
			onAdded?.(outcome.subject);
			return true;
		case "alreadyExists":
			showWarning(
				`Project '${outcome.subject.title}' already exists`,
				`The project at "${outcome.subject.path}" is already added`,
				{
					label: "Open project",
					testId: TestId.AddProjectAlreadyExistsModalOpenProjectButton,
					onClick: (dismiss) => {
						goto(projectPath(outcome.subject.id));
						dismiss();
					},
				},
				TestId.AddProjectAlreadyExistsModal,
			);
			return true;
		case "pathNotFound":
			showWarning("Path not found", "The specified path does not exist on the filesystem.");
			return true;
		case "notADirectory":
			showWarning("Not a directory", "The specified path is not a directory.");
			return true;
		case "bareRepository":
			showToast({
				testId: TestId.AddProjectBareRepoModal,
				style: "danger",
				title: "Bare repository",
				message: "The specified path appears to be a bare Git repository and cannot be added.",
			});
			return true;
		case "nonMainWorktree":
			showWarning(
				"Non-main worktree",
				"The specified path is not the main worktree of the repository.",
			);
			return true;
		case "noWorkdir":
			showWarning("No workdir", "The specified repository does not have a working directory.");
			return true;
		case "noDotGitDirectory":
			showWarning(
				"No .git directory",
				"The specified path does not contain a .git directory.",
				undefined,
				TestId.AddProjectNoDotGitDirectoryModal,
			);
			return true;
		case "reftableRefFormatUnsupported":
			showWarning(
				"Unsupported reference format",
				"GitButler does not support repositories using the reftable reference format yet.",
			);
			return true;
		case "notAGitRepository":
			showWarning(
				"Not a Git repository",
				`Unable to add project: ${outcome.subject}`,
				undefined,
				TestId.AddProjectNotAGitRepoModal,
			);
			return true;
	}
}
