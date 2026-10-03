import type { FC } from "react";
import { useSuspenseQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { AddProjectButton } from "#ui/components/AddProjectButton.tsx";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import { LiteTestId } from "#ui/testIds.ts";
import styles from "./IndexPage.module.css";

export const IndexPage: FC = () =>
	window.lite.hosted === true ? <PublishedProjects /> : <Onboarding />;

const Onboarding: FC = () => {
	const { addLocalRepository, isPending } = useAddLocalRepository();

	return (
		<section className={styles.page} data-testid={LiteTestId.OnboardingPage}>
			<h1>Welcome to GitButler Next Nightly</h1>
			<p>Add a local Git repository to get started.</p>
			<AddProjectButton isPending={isPending} onClick={() => void addLocalRepository()} />
		</section>
	);
};

/** What a hosted server holds for the signed-in account: the projects it published. */
const PublishedProjects: FC = () => {
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);

	return (
		<section className={styles.page}>
			<h1>Your published projects</h1>
			{projects.length === 0 ? (
				<p>Nothing yet. Publish a branch from GitButler on your machine to see it here.</p>
			) : (
				<ul className={styles.projects}>
					{projects.map((project) => (
						<li key={project.id}>
							<Link
								to="/project/$id/workspace"
								params={{ id: project.id }}
								className={styles.project}
							>
								{project.title}
							</Link>
						</li>
					))}
				</ul>
			)}
		</section>
	);
};
