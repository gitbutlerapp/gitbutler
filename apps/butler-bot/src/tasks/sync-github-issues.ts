import type { Task } from '@/types';
import { createEmbedding, parseEmbedding, stringifyEmbedding } from '@/utils/embedding';

export const syncGithubIssues: Task = {
	name: 'sync-github-issues',
	schedule: '0 0 * * *', // Run every day at midnight
	execute: async ({ prisma, octokit, openai }) => {
		try {
			// Get all repositories from the database
			const repositories = await prisma.gitHubRepo.findMany();

			if (repositories.length === 0) {
				// eslint-disable-next-line no-console
				console.log('No repositories to sync issues from');
				return;
			}

			// Log the start of the sync process
			// eslint-disable-next-line no-console
			console.log(`Starting GitHub issues sync for ${repositories.length} repositories`);
			let totalIssuesProcessed = 0;
			let totalIssuesAdded = 0;
			let totalIssuesUpdated = 0;

			// Process each repository
			for (const repo of repositories) {
				try {
					// eslint-disable-next-line no-console
					console.log(`Syncing issues for ${repo.owner}/${repo.name}`);

					// Fetch open issues for the repository
					const issues = await octokit.paginate(octokit.rest.issues.listForRepo, {
						owner: repo.owner,
						repo: repo.name,
						state: 'open'
					});

					// eslint-disable-next-line no-console
					console.log(`Found ${issues.length} open issues for ${repo.owner}/${repo.name}`);
					totalIssuesProcessed += issues.length;

					// Process each issue
					for (const issue of issues) {
						// Skip pull requests (they're also returned by the issues API)
						if (issue.pull_request) {
							continue;
						}

						// Check if the issue already exists in our database
						const existingIssue = await prisma.gitHubIssue.findFirst({
							where: {
								github_repo_id: repo.id,
								issue_number: issue.number
							}
						});

						if (existingIssue) {
							// Update the existing issue if the title changed
							if (
								existingIssue.title !== issue.title ||
								!existingIssue.url ||
								!existingIssue.embedding ||
								parseEmbedding(existingIssue.embedding).length !== 3072
							) {
								const embedding = await createEmbedding(
									openai,
									`${issue.title}\n\n${issue.body ?? ''}`.slice(0, 2000)
								);
								await prisma.gitHubIssue.update({
									where: { id: existingIssue.id },
									data: {
										title: issue.title,
										embedding: stringifyEmbedding(embedding),
										url: issue.html_url,
										updated_at: new Date()
									}
								});
								totalIssuesUpdated++;
							}
						} else {
							const embedding = await createEmbedding(
								openai,
								`${issue.title}\n\n${issue.body ?? ''}`.slice(0, 2000)
							);

							// Create a new issue
							await prisma.gitHubIssue.create({
								data: {
									title: issue.title,
									issue_number: issue.number,
									github_repo_id: repo.id,
									embedding: stringifyEmbedding(embedding),
									url: issue.html_url
								}
							});
							totalIssuesAdded++;
						}
					}
				} catch (repoError) {
					console.error(`Error processing repository ${repo.owner}/${repo.name}:`, repoError);
				}
			}

			// eslint-disable-next-line no-console
			console.log(`GitHub issues sync completed`);
			// eslint-disable-next-line no-console
			console.log(`Processed ${totalIssuesProcessed} issues`);
			// eslint-disable-next-line no-console
			console.log(`Added ${totalIssuesAdded} new issues`);
			// eslint-disable-next-line no-console
			console.log(`Updated ${totalIssuesUpdated} existing issues`);
		} catch (error) {
			console.error('Error syncing GitHub issues:', error);
		}
	}
};
