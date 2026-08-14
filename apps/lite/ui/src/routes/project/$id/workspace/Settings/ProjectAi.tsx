import { useSuspenseQueries, useQueryClient } from "@tanstack/react-query";
import { useState, type FC } from "react";
import { aiConfigurationQueryOptions } from "#ui/api/queries.ts";
import { Switch } from "#ui/components/Switch.tsx";
import {
	DEFAULT_COMMIT_MESSAGE_PROMPT,
	projectAiSettingsQueryOptions,
	writeProjectAiSettings,
	type ProjectAiSettings,
} from "#ui/project-ai-settings.ts";
import styles from "./ProjectAi.module.css";
import { Row, Section } from "./Section.tsx";

export const ProjectAi: FC<{ projectId: string }> = ({ projectId }) => {
	const [{ data: configuration }, { data: stored }] = useSuspenseQueries({
		queries: [aiConfigurationQueryOptions, projectAiSettingsQueryOptions(projectId)],
	});
	const client = useQueryClient();
	const [prompt, setPrompt] = useState(stored.commitMessagePrompt);

	const save = (update: Partial<ProjectAiSettings>) => {
		const settings = {
			...(client.getQueryData(projectAiSettingsQueryOptions(projectId).queryKey) ?? stored),
			...update,
		};
		writeProjectAiSettings(projectId, settings);
		client.setQueryData(projectAiSettingsQueryOptions(projectId).queryKey, settings);
	};
	const savePrompt = () => {
		const commitMessagePrompt =
			prompt.trim().length > 0 ? prompt.trim() : DEFAULT_COMMIT_MESSAGE_PROMPT;
		setPrompt(commitMessagePrompt);
		save({ commitMessagePrompt });
	};

	return (
		<Section>
			<Row
				label="Commit message generation"
				labelId="project-ai-enabled"
				hint={
					configuration.isConfigured
						? "Sends selected file diffs to the configured provider when you generate a message."
						: "Configure credentials for a provider under Application → AI first."
				}
			>
				<Switch
					aria-labelledby="project-ai-enabled"
					checked={stored.enabled && configuration.isConfigured}
					disabled={!configuration.isConfigured}
					onCheckedChange={(enabled) => save({ enabled })}
				/>
			</Row>

			{stored.enabled && configuration.isConfigured && (
				<Row
					label="Commit prompt"
					htmlFor="project-ai-commit-prompt"
					hint="The selected file diffs are appended automatically."
					stacked
				>
					<textarea
						id="project-ai-commit-prompt"
						className={styles.prompt}
						value={prompt}
						onChange={(event) => setPrompt(event.currentTarget.value)}
						onBlur={savePrompt}
					/>
				</Row>
			)}
		</Section>
	);
};
