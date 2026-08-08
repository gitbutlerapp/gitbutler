import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC } from "react";
import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { useUpdateProjectSettings } from "#ui/api/mutations.ts";
import { Switch } from "#ui/components/Switch.tsx";
import { assert } from "#ui/assert.ts";
import { changing } from "./project-settings.ts";
import { Row, Section } from "./Section.tsx";

export const ProjectExperimental: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = assert(projects.find((candidate) => candidate.id === projectId));
	const { mutate: updateProjectSettings } = useUpdateProjectSettings();

	return (
		<Section>
			<Row
				label="Ignore host certificate checks"
				labelId="omit-certificate-check"
				hint="Skips certificate verification when authenticating over SSH."
			>
				<Switch
					aria-labelledby="omit-certificate-check"
					checked={project.omit_certificate_check ?? false}
					onCheckedChange={(omitCertificateCheck) =>
						updateProjectSettings({
							projectId,
							settings: changing({ omitCertificateCheck }),
						})
					}
				/>
			</Row>
		</Section>
	);
};
