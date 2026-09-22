import { useSuspenseQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState, type FC, type ReactNode } from "react";
import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { useDeleteProject, useUpdateProjectSettings } from "#ui/api/mutations.ts";
import { getButtonClassName } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { FieldControlStyles, FieldTextareaStyles } from "@gitbutler/ui-react/Field.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { assert } from "#ui/assert.ts";
import { revealInFolderLabel } from "#ui/hotkeys.ts";
import { useCopied } from "#ui/components/useCopied.ts";
import { IconButton } from "./IconButton.tsx";
import styles from "./Project.module.css";
import { changing } from "./project-settings.ts";
import { Row, Section } from "./Section.tsx";

/** A control with a line under it, at the row's end: the field, then what it is for. */
const Stack: FC<{ note?: ReactNode; children: ReactNode }> = (p) => (
	<div className={styles.stack}>
		{p.children}
		{p.note !== undefined && (
			<span className={classes("text-12", "text-body", styles.note)}>{p.note}</span>
		)}
	</div>
);

export const Project: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = assert(projects.find((candidate) => candidate.id === projectId));
	const { mutate: updateProjectSettings } = useUpdateProjectSettings(projectId);
	const { isPending: isRemoving, mutate: deleteProject } = useDeleteProject(projectId);
	const navigate = useNavigate();
	const { copied, copy: copyPath } = useCopied(project.path);

	// Held locally so a refetch cannot interrupt typing; committed on blur or Enter.
	const [title, setTitle] = useState(project.title);
	const [description, setDescription] = useState(project.description ?? "");
	const [confirmingRemove, setConfirmingRemove] = useState(false);

	const removeProject = () =>
		deleteProject(projectId, {
			// The route this dialog lives in is gone, so leave before it notices.
			onSuccess: () => void navigate({ to: "/" }),
		});

	const saveTitle = () => {
		// The picker and window title both read this, so an empty one is not useful.
		if (title.trim() === "") setTitle(project.title);
		else updateProjectSettings({ projectId, settings: changing({ title }) });
	};

	const saveDescription = () =>
		updateProjectSettings({ projectId, settings: changing({ description }) });

	return (
		<>
			<Section>
				<Row label="Name" htmlFor="project-title" wide>
					<Stack note="How this repository is labelled inside GitButler">
						<FieldControlStyles
							id="project-title"
							type="text"
							value={title}
							onChange={(evt) => setTitle(evt.currentTarget.value)}
							onBlur={saveTitle}
							onKeyDown={(evt) => evt.key === "Enter" && saveTitle()}
						/>
					</Stack>
				</Row>

				<Row label="Description" htmlFor="project-description" wide>
					<Stack>
						<FieldTextareaStyles
							id="project-description"
							className={styles.description}
							placeholder="About the project"
							value={description}
							onChange={(evt) => setDescription(evt.currentTarget.value)}
							onBlur={saveDescription}
						/>
					</Stack>
				</Row>

				<Row label="Path" wide>
					<Stack note="Where the repository lives. Set when the project was added.">
						<div className={styles.path}>
							<FieldControlStyles type="text" aria-label="Path" value={project.path} disabled />
							<IconButton label={copied ? "Copied" : "Copy path"} onClick={copyPath}>
								<Icon name={copied ? "tick" : "copy"} />
							</IconButton>
							<IconButton
								label={revealInFolderLabel}
								className={styles.reveal}
								onClick={() => void window.lite.showItemInFolder(project.path)}
							>
								<Icon name="folder" />
								<Icon name="arrow-up-right" />
							</IconButton>
						</div>
					</Stack>
				</Row>
			</Section>

			<Section>
				<Row
					label="Remove project"
					hint="Forgets its GitButler configuration. The repository on disk is untouched."
				>
					{confirmingRemove ? (
						<div className={styles.confirm}>
							<button
								type="button"
								className={getButtonClassName({ variant: "danger" })}
								disabled={isRemoving}
								onClick={removeProject}
							>
								{isRemoving ? "Removing…" : "Confirm"}
							</button>
							<button
								type="button"
								className={getButtonClassName({})}
								disabled={isRemoving}
								onClick={() => setConfirmingRemove(false)}
							>
								Cancel
							</button>
						</div>
					) : (
						<button
							type="button"
							className={getButtonClassName({ variant: "danger" })}
							onClick={() => setConfirmingRemove(true)}
						>
							<Icon name="bin" />
							Remove…
						</button>
					)}
				</Row>
			</Section>
		</>
	);
};
